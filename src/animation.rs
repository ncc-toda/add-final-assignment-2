//! 背景アニメーションのパーティクルシミュレーション。
//!
//! 雨・雪・晴れ(きらめき)・曇り(雲の横流れ)・雷(雨+稲妻)を
//! 1つのパーティクルエンジンで表現する。描画(文字の配置)は `tui::view` が担い、
//! ここでは座標・文字・色分類の計算のみを行う(TDD対象)。

use crate::config::AnimationConfig;
use crate::weather_code::WeatherCategory;

/// `config.density` の上限(これ以上は画面が文字で埋まるだけのため)。
const DENSITY_MAX: f64 = 3.0;
/// `config.speed` の許容範囲。0だと完全静止するため下限を設ける。
const SPEED_MIN: f64 = 0.1;
const SPEED_MAX: f64 = 5.0;
/// 稲妻の平均発生間隔(tick数。50ms/tickなので約1秒に1回)。
const BOLT_INTERVAL_TICKS: u64 = 22;
/// 稲妻が表示され続けるtick数。
const BOLT_LIFETIME: u8 = 4;

/// パーティクルの色分類。実際の色はテーマ非依存でview側が固定的に割り当てる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleColor {
    Rain,
    Snow,
    Sun,
    /// 晴れのきらめきのうち、白く強く光る点(金色の `Sun` と混在させてシマー感を出す)
    Spark,
    Cloud,
    Lightning,
}

/// 描画用のパーティクル1点(セル座標・文字・色分類)。
pub type Glyph = (u16, u16, &'static str, ParticleColor);

/// シード指定可能な自前PRNG(xorshift64)。見た目のランダムさ用途で十分。
#[derive(Debug, Clone)]
pub struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    pub fn new(seed: u64) -> Self {
        // xorshiftは状態0だと0しか生成しないため回避する
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// [0.0, 1.0) の一様乱数。
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// [lo, hi) の一様乱数。
    fn range_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.next_f64() * (hi - lo)
    }
}

/// カテゴリごとの基本密度(セルあたりのパーティクル数)。
fn base_rate(category: WeatherCategory) -> f64 {
    match category {
        WeatherCategory::Rain | WeatherCategory::Thunder => 0.03,
        WeatherCategory::Snow => 0.022,
        WeatherCategory::Cloudy => 0.014,
        WeatherCategory::Sunny => 0.01,
    }
}

/// 稲妻セルの進行方向(dx)に応じた線分グリフ。
fn bolt_glyph(dx: i64) -> &'static str {
    match dx {
        d if d < 0 => "╱",
        d if d > 0 => "╲",
        _ => "┃",
    }
}

/// 降水系カテゴリか(降水確率係数を適用する対象か)。
fn is_precipitation(category: WeatherCategory) -> bool {
    matches!(
        category,
        WeatherCategory::Rain | WeatherCategory::Snow | WeatherCategory::Thunder
    )
}

/// パーティクル数を計算する。
///
/// パーティクル数 = セル数 × カテゴリ基本率 × 降水係数 × density(0〜3にクランプ)。
/// 降水係数は雨/雪/雷のみ `0.3 + 0.7 × (降水確率 / 100)`(確率不明時は1.0)。
pub fn particle_count(
    width: u16,
    height: u16,
    category: WeatherCategory,
    max_pop: Option<u8>,
    density: f64,
) -> usize {
    let cells = f64::from(width) * f64::from(height);
    let pop_factor = if is_precipitation(category) {
        max_pop.map_or(1.0, |p| 0.3 + 0.7 * f64::from(p.min(100)) / 100.0)
    } else {
        1.0
    };
    let density = density.clamp(0.0, DENSITY_MAX);
    (cells * base_rate(category) * pop_factor * density).round() as usize
}

/// パーティクル1つ分の状態。
#[derive(Debug, Clone)]
struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    glyph: &'static str,
    color: ParticleColor,
    /// 雪のゆらぎ・晴れの明滅の位相ずらし
    phase: f64,
    /// 晴れの明滅周期(tick数)。明滅しないパーティクルは0
    blink_period: u64,
}

/// 稲妻1本。生成時にギザギザの経路(本体+枝)をセル列として確定させ、
/// 数tickだけ表示する。各セルは座標と方向に応じたグリフを持つ。
#[derive(Debug, Clone)]
struct Bolt {
    /// 稲妻を構成するセル(x, y, グリフ)の列。本体と枝を含む。
    cells: Vec<(u16, u16, &'static str)>,
    remaining: u8,
}

/// 背景アニメーション全体の状態。tickごとに `tick()` を呼び `glyphs()` で描画内容を得る。
#[derive(Debug, Clone)]
pub struct ParticleField {
    category: WeatherCategory,
    particles: Vec<Particle>,
    bolts: Vec<Bolt>,
    rng: Xorshift64,
    width: u16,
    height: u16,
    max_pop: Option<u8>,
    speed: f64,
    density: f64,
    tick_count: u64,
}

impl ParticleField {
    pub fn new(
        seed: u64,
        category: WeatherCategory,
        max_pop: Option<u8>,
        config: &AnimationConfig,
        width: u16,
        height: u16,
    ) -> Self {
        let mut field = Self {
            category,
            particles: Vec::new(),
            bolts: Vec::new(),
            rng: Xorshift64::new(seed),
            width,
            height,
            max_pop,
            speed: config.speed.clamp(SPEED_MIN, SPEED_MAX),
            density: config.density.clamp(0.0, DENSITY_MAX),
            tick_count: 0,
        };
        field.scatter();
        field
    }

    /// 端末サイズ変更に追従する(パーティクルを撒き直す)。同サイズなら何もしない。
    pub fn resize(&mut self, width: u16, height: u16) {
        if width == self.width && height == self.height {
            return;
        }
        self.width = width;
        self.height = height;
        self.bolts.clear();
        self.scatter();
    }

    /// パーティクルを目標数まで画面全体にランダム配置し直す。
    fn scatter(&mut self) {
        let count = particle_count(
            self.width,
            self.height,
            self.category,
            self.max_pop,
            self.density,
        );
        self.particles.clear();
        for _ in 0..count {
            let particle = self.spawn(false);
            self.particles.push(particle);
        }
    }

    /// パーティクルを1つ生成する。`at_top` なら画面上端(落下系の再投入用)。
    fn spawn(&mut self, at_top: bool) -> Particle {
        let x = self.rng.range_f64(0.0, f64::from(self.width.max(1)));
        let y = if at_top {
            0.0
        } else {
            self.rng.range_f64(0.0, f64::from(self.height.max(1)))
        };
        let phase = self.rng.range_f64(0.0, std::f64::consts::TAU);
        match self.category {
            WeatherCategory::Rain | WeatherCategory::Thunder => {
                // 風で右に流れる斜めの雨。落下速度に幅を持たせて奥行き感を出し、
                // 速い(手前の)雨ほど太いグリフにする。
                let vy = self.rng.range_f64(0.4, 1.2);
                let glyph = if vy > 0.95 {
                    "┃"
                } else if vy > 0.65 {
                    "╲"
                } else {
                    "│"
                };
                Particle {
                    x,
                    y,
                    vx: self.rng.range_f64(0.12, 0.30),
                    vy,
                    glyph,
                    color: ParticleColor::Rain,
                    phase,
                    blink_period: 0,
                }
            }
            WeatherCategory::Snow => Particle {
                x,
                y,
                vx: 0.0,
                vy: self.rng.range_f64(0.08, 0.28),
                glyph: match self.rng.next_u64() % 5 {
                    0 => "❄",
                    1 => "❆",
                    2 => "✻",
                    3 => "*",
                    _ => "·",
                },
                color: ParticleColor::Snow,
                phase,
                blink_period: 0,
            },
            WeatherCategory::Cloudy => Particle {
                x,
                y,
                vx: self.rng.range_f64(0.03, 0.12),
                vy: 0.0,
                // 濃淡のあるブロック文字で雲の塊のようなテクスチャにする
                glyph: match self.rng.next_u64() % 5 {
                    0 => "▓",
                    1 => "▒",
                    2 => "░",
                    3 => "●",
                    _ => "○",
                },
                color: ParticleColor::Cloud,
                phase,
                blink_period: 0,
            },
            WeatherCategory::Sunny => Particle {
                x,
                y,
                vx: 0.0,
                vy: 0.0,
                glyph: match self.rng.next_u64() % 6 {
                    0 => "✦",
                    1 => "✧",
                    2 => "✶",
                    3 => "⋆",
                    4 => "+",
                    _ => "·",
                },
                // 一部を白く強く光らせ、金色と混ざってシマーする
                color: if self.rng.next_u64().is_multiple_of(3) {
                    ParticleColor::Spark
                } else {
                    ParticleColor::Sun
                },
                phase,
                // 明滅を速めて活発なきらめきにする
                blink_period: 10 + (self.rng.next_u64() % 20),
            },
        }
    }

    /// 1tick分の状態更新(位置・稲妻の発生と消滅)。
    pub fn tick(&mut self) {
        self.tick_count += 1;
        let (w, h) = (f64::from(self.width.max(1)), f64::from(self.height.max(1)));
        let speed = self.speed;
        let t = self.tick_count as f64;

        let mut respawn_indices = Vec::new();
        for (i, p) in self.particles.iter_mut().enumerate() {
            p.y += p.vy * speed;
            match p.color {
                ParticleColor::Snow => {
                    // ゆらゆら横に揺れながら落ちる
                    p.x += (t * 0.15 + p.phase).sin() * 0.2 * speed;
                }
                ParticleColor::Cloud => {
                    p.x += p.vx * speed;
                }
                ParticleColor::Rain => {
                    // 風で斜めに流れる
                    p.x += p.vx * speed;
                }
                _ => {}
            }
            // 横方向は画面端でループ
            p.x = p.x.rem_euclid(w);
            if p.y >= h {
                respawn_indices.push(i);
            }
        }
        // 画面下に抜けた落下系パーティクルは上端から再投入
        for i in respawn_indices {
            self.particles[i] = self.spawn(true);
        }

        // 稲妻: 表示中のものを減衰させ、いなければ確率的に発生
        for bolt in &mut self.bolts {
            bolt.remaining -= 1;
        }
        self.bolts.retain(|b| b.remaining > 0);
        if self.category == WeatherCategory::Thunder
            && self.bolts.is_empty()
            && self.height >= 4
            && self.rng.next_u64().is_multiple_of(BOLT_INTERVAL_TICKS)
        {
            let bolt = self.build_bolt();
            self.bolts.push(bolt);
        }
    }

    /// ギザギザに枝分かれする稲妻を1本生成する。上端付近から下へ、
    /// 左右にジグザグしながら伸ばし、途中から短い枝を分岐させる。
    fn build_bolt(&mut self) -> Bolt {
        let w = i64::from(self.width.max(1));
        let max_len = (self.height * 3 / 4).max(4);
        let len = 4 + (self.rng.next_u64() % u64::from(max_len - 3)) as u16;
        let mut cells = Vec::new();

        let mut x = (self.rng.next_u64() % w as u64) as i64;
        for y in 0..len {
            // -1 / 0 / +1 に振れるが、まっすぐ進みやすくする
            let dx = match self.rng.next_u64() % 4 {
                0 => -1,
                1 => 1,
                _ => 0,
            };
            let glyph = bolt_glyph(dx);
            let cx = x.clamp(0, w - 1);
            cells.push((cx as u16, y, glyph));

            // まれに横向きの枝を生やす(画面中ほど限定)
            if y > 1 && y + 1 < len && self.rng.next_u64().is_multiple_of(5) {
                let bdir: i64 = if self.rng.next_u64().is_multiple_of(2) {
                    1
                } else {
                    -1
                };
                let blen = 2 + (self.rng.next_u64() % 3) as u16;
                let mut bx = cx;
                for k in 1..=blen {
                    bx = (bx + bdir).clamp(0, w - 1);
                    let by = y + k;
                    if by >= len {
                        break;
                    }
                    cells.push((bx as u16, by, bolt_glyph(bdir)));
                }
            }

            x = (x + dx).clamp(0, w - 1);
        }

        Bolt {
            cells,
            remaining: BOLT_LIFETIME,
        }
    }

    /// 落雷した直後の数tickかどうか(画面フラッシュ演出用)。
    pub fn flash_active(&self) -> bool {
        self.bolts
            .iter()
            .any(|b| b.remaining >= BOLT_LIFETIME - 1)
    }

    /// 現在表示すべきパーティクルの一覧(画面内のもののみ)。
    pub fn glyphs(&self) -> Vec<Glyph> {
        let mut out = Vec::with_capacity(self.particles.len());
        for p in &self.particles {
            // 晴れのきらめきは周期的に明滅する(周期の1/3は消灯)
            if p.blink_period > 0 {
                let period = p.blink_period;
                let on = (self.tick_count + p.phase.to_bits() % period) % period < period * 2 / 3;
                if !on {
                    continue;
                }
            }
            let (x, y) = (p.x.floor() as i64, p.y.floor() as i64);
            if (0..i64::from(self.width)).contains(&x) && (0..i64::from(self.height)).contains(&y) {
                out.push((x as u16, y as u16, p.glyph, p.color));
            }
        }
        for bolt in &self.bolts {
            for &(x, y, glyph) in &bolt.cells {
                if x < self.width && y < self.height {
                    out.push((x, y, glyph, ParticleColor::Lightning));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(speed: f64, density: f64) -> AnimationConfig {
        AnimationConfig {
            enabled: true,
            speed,
            density,
        }
    }

    // ===== particle_count =====

    #[test]
    fn 雨は降水確率係数込みで計算される() {
        // 4000セル × 0.03 × (0.3 + 0.7×0.7) × 1.0 = 94.8 → 95
        assert_eq!(
            particle_count(100, 40, WeatherCategory::Rain, Some(70), 1.0),
            95
        );
    }

    #[test]
    fn 降水確率不明なら係数は1になる() {
        // 4000 × 0.03 × 1.0 × 1.0 = 120
        assert_eq!(
            particle_count(100, 40, WeatherCategory::Rain, None, 1.0),
            120
        );
    }

    #[test]
    fn 晴れと曇りは降水確率の影響を受けない() {
        let sunny_low = particle_count(100, 40, WeatherCategory::Sunny, Some(0), 1.0);
        let sunny_high = particle_count(100, 40, WeatherCategory::Sunny, Some(100), 1.0);
        assert_eq!(sunny_low, sunny_high);
        assert_eq!(sunny_low, 40); // 4000 × 0.01

        let cloudy = particle_count(100, 40, WeatherCategory::Cloudy, None, 1.0);
        assert_eq!(cloudy, 56); // 4000 × 0.014
    }

    #[test]
    fn density設定が乗算されクランプされる() {
        let base = particle_count(100, 40, WeatherCategory::Rain, None, 1.0);
        assert_eq!(
            particle_count(100, 40, WeatherCategory::Rain, None, 2.0),
            base * 2
        );
        // 3.0を超えても3.0でクランプ
        assert_eq!(
            particle_count(100, 40, WeatherCategory::Rain, None, 99.0),
            particle_count(100, 40, WeatherCategory::Rain, None, 3.0),
        );
        assert_eq!(particle_count(100, 40, WeatherCategory::Rain, None, 0.0), 0);
    }

    // ===== ParticleField =====

    #[test]
    fn 生成直後のパーティクルは全て画面内にある() {
        let field = ParticleField::new(
            42,
            WeatherCategory::Rain,
            Some(50),
            &config(1.0, 1.0),
            80,
            24,
        );
        let glyphs = field.glyphs();
        assert!(!glyphs.is_empty());
        for (x, y, _, _) in &glyphs {
            assert!(*x < 80 && *y < 24, "out of bounds: ({x}, {y})");
        }
    }

    #[test]
    fn 雨パーティクルはtickで下に落ちる() {
        let mut field =
            ParticleField::new(42, WeatherCategory::Rain, None, &config(1.0, 1.0), 80, 24);
        let before: Vec<f64> = field.particles.iter().map(|p| p.y).collect();
        field.tick();
        // 大半は下方へ移動し、下端を抜けたものだけが上端へ再投入される。
        let (mut down, mut reset) = (0, 0);
        for (p, y0) in field.particles.iter().zip(&before) {
            if p.y > *y0 {
                down += 1;
            } else if p.y < *y0 {
                reset += 1;
            }
        }
        assert!(down > reset, "down: {down}, reset: {reset}");
    }

    #[test]
    fn 長時間tickしてもパーティクルは画面内に再投入され続ける() {
        let mut field = ParticleField::new(
            7,
            WeatherCategory::Snow,
            Some(80),
            &config(1.0, 1.0),
            40,
            12,
        );
        let count = field.particles.len();
        for _ in 0..500 {
            field.tick();
        }
        assert_eq!(field.particles.len(), count, "パーティクル数は不変");
        for (x, y, _, _) in field.glyphs() {
            assert!(x < 40 && y < 12, "out of bounds: ({x}, {y})");
        }
    }

    #[test]
    fn 雪パーティクルは横にゆらぐ() {
        let mut field =
            ParticleField::new(42, WeatherCategory::Snow, None, &config(1.0, 1.0), 80, 24);
        let before: Vec<f64> = field.particles.iter().map(|p| p.x).collect();
        for _ in 0..10 {
            field.tick();
        }
        let moved = field
            .particles
            .iter()
            .zip(&before)
            .any(|(p, x0)| (p.x - x0).abs() > f64::EPSILON);
        assert!(moved, "雪が横方向に動いていない");
    }

    #[test]
    fn 曇りパーティクルは横に流れる() {
        let mut field =
            ParticleField::new(42, WeatherCategory::Cloudy, None, &config(1.0, 1.0), 80, 24);
        let before: Vec<f64> = field.particles.iter().map(|p| p.x).collect();
        for _ in 0..20 {
            field.tick();
        }
        let all_moved = field
            .particles
            .iter()
            .zip(&before)
            .all(|(p, x0)| (p.x - x0).abs() > f64::EPSILON || p.vx == 0.0);
        assert!(all_moved);
    }

    #[test]
    fn 晴れパーティクルは明滅する() {
        let mut field =
            ParticleField::new(42, WeatherCategory::Sunny, None, &config(1.0, 1.0), 80, 24);
        let total = field.particles.len();
        assert!(total > 0);
        // 十分な期間観測すると「全点灯」でないtickが存在する(=明滅している)
        let mut saw_partial = false;
        for _ in 0..100 {
            field.tick();
            if field.glyphs().len() < total {
                saw_partial = true;
                break;
            }
        }
        assert!(saw_partial, "きらめきが明滅していない");
    }

    #[test]
    fn 雷カテゴリではやがて稲妻が発生し数tickで消える() {
        let mut field = ParticleField::new(
            42,
            WeatherCategory::Thunder,
            Some(90),
            &config(1.0, 1.0),
            80,
            24,
        );
        let mut bolt_seen = false;
        for _ in 0..500 {
            field.tick();
            if field
                .glyphs()
                .iter()
                .any(|(_, _, _, c)| *c == ParticleColor::Lightning)
            {
                bolt_seen = true;
                break;
            }
        }
        assert!(bolt_seen, "500tick以内に稲妻が発生しなかった");

        // 稲妻は数tickで消える
        let mut gone = false;
        for _ in 0..(BOLT_LIFETIME as usize + 1) {
            field.tick();
            if field
                .glyphs()
                .iter()
                .all(|(_, _, _, c)| *c != ParticleColor::Lightning)
            {
                gone = true;
                break;
            }
        }
        assert!(gone, "稲妻が消えない");
    }

    #[test]
    fn 稲妻はギザギザで発生直後はフラッシュする() {
        let mut field = ParticleField::new(
            42,
            WeatherCategory::Thunder,
            Some(90),
            &config(1.0, 1.0),
            80,
            24,
        );
        // 稲妻が発生するまで進める
        let mut struck = false;
        for _ in 0..500 {
            field.tick();
            let bolt_cells: Vec<_> = field
                .glyphs()
                .into_iter()
                .filter(|(_, _, _, c)| *c == ParticleColor::Lightning)
                .collect();
            if !bolt_cells.is_empty() {
                // 発生直後の数tickはフラッシュが立つ
                assert!(field.flash_active(), "落雷直後はフラッシュするはず");
                // 直線の棒ではなく、複数のx座標にまたがるギザギザである
                let xs: std::collections::HashSet<u16> =
                    bolt_cells.iter().map(|(x, _, _, _)| *x).collect();
                assert!(xs.len() >= 2, "稲妻が直線的すぎる: xs={xs:?}");
                struck = true;
                break;
            }
        }
        assert!(struck, "500tick以内に稲妻が発生しなかった");
    }

    #[test]
    fn resizeで新しい画面サイズに収まる() {
        let mut field =
            ParticleField::new(42, WeatherCategory::Rain, None, &config(1.0, 1.0), 100, 40);
        field.resize(20, 10);
        for _ in 0..50 {
            field.tick();
        }
        for (x, y, _, _) in field.glyphs() {
            assert!(x < 20 && y < 10, "out of bounds: ({x}, {y})");
        }
    }

    #[test]
    fn 同じシードなら同じ結果になる() {
        let make = || {
            let mut f = ParticleField::new(
                123,
                WeatherCategory::Thunder,
                Some(60),
                &config(1.0, 1.0),
                60,
                20,
            );
            for _ in 0..100 {
                f.tick();
            }
            f.glyphs()
        };
        assert_eq!(make(), make());
    }

    /// 1tick後に再投入されなかったパーティクルの「変位 ÷ 固有速度」
    /// (=適用されたspeed倍率)を返す。
    fn applied_speed(field: &mut ParticleField) -> f64 {
        let before: Vec<(f64, f64)> = field.particles.iter().map(|p| (p.y, p.vy)).collect();
        field.tick();
        field
            .particles
            .iter()
            .zip(&before)
            .find_map(|(p, (y0, vy))| (p.y > *y0).then(|| (p.y - y0) / vy))
            .expect("再投入されなかったパーティクルが1つはあるはず")
    }

    #[test]
    fn speed設定は下限にクランプされ落下速度の倍率になる() {
        // 0.0は下限0.1にクランプされる
        let mut slow =
            ParticleField::new(9, WeatherCategory::Rain, None, &config(0.0, 1.0), 80, 24);
        let mut fast =
            ParticleField::new(9, WeatherCategory::Rain, None, &config(3.0, 1.0), 80, 24);
        assert!((applied_speed(&mut slow) - 0.1).abs() < 1e-9);
        assert!((applied_speed(&mut fast) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn 乱数は決定論的で偏りすぎない() {
        let mut rng = Xorshift64::new(1);
        let mut rng2 = Xorshift64::new(1);
        assert_eq!(rng.next_u64(), rng2.next_u64());

        let mut rng = Xorshift64::new(0); // 0シードでも動く
        let vals: Vec<f64> = (0..1000).map(|_| rng.next_f64()).collect();
        assert!(vals.iter().all(|v| (0.0..1.0).contains(v)));
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        assert!((0.4..0.6).contains(&mean), "mean: {mean}");
    }
}
