use core::fmt;
use std::{
    cmp::Ordering,
    f64::consts,
    fs::File,
    io::{self, BufRead, BufReader},
    path::Path,
    time::Duration,
};

use crossterm::{
    event::{poll, read, Event, KeyCode, KeyEventKind},
    execute,
    style::Stylize,
    terminal::{self, ClearType},
};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

#[derive(Clone)]
enum Tile {
    Wall(char),
    Air,
}

impl Tile {
    fn obstructing(&self) -> bool {
        match self {
            Tile::Wall(_) => true,
            Tile::Air => false,
        }
    }
}

struct Map {
    width: usize,
    tiles: Vec<Tile>,
}

impl Map {
    fn new(w: usize, h: usize) -> Self {
        Self {
            tiles: vec![Tile::Air; w * h],
            width: w,
        }
    }

    fn from_file<P: AsRef<Path>>(path: P) -> Self {
        let file = File::open(path).expect("cannot open file");
        let reader = BufReader::new(file);

        let lines: Vec<_> = reader
            .lines()
            .map(|line| line.expect("cannot read line"))
            .collect();

        let first_line = lines.first().expect("empty file");
        let width = first_line.len();

        let mut map = Self::new(width, lines.len());
        for (li, line) in lines.iter().enumerate() {
            for (i, c) in line.chars().enumerate() {
                let tile = match c {
                    ' ' => continue,
                    c => Tile::Wall(c),
                };
                map.tiles[i + li * width] = tile;
            }
        }
        map
    }

    fn at(&self, x: usize, y: usize) -> &Tile {
        &self.tiles[y * self.width + x]
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.tiles.len() / self.width
    }
}

impl fmt::Display for Map {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, tile) in self.tiles.iter().enumerate() {
            let c = match tile {
                Tile::Wall(c) => c.blue(),
                Tile::Air => '.'.yellow(),
            };
            write!(f, "{}", c)?;
            if (index + 1) % self.width == 0 {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

fn distance(a: &Pos<u32>, b: &Pos<u32>) -> f64 {
    let dx = a.x as i32 - b.x as i32;
    let dy = a.y as i32 - b.y as i32;

    let dxs = dx * dx;
    let dys = dy * dy;

    ((dxs + dys) as f64).sqrt()
}

#[derive(PartialEq)]
struct Pos<T> {
    x: T,
    y: T,
}

impl<T> Pos<T> {
    fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
}

struct LightSpec {
    radius: f64,
    width: f64,
}

impl LightSpec {
    fn new(radius: f64, width: f64) -> Self {
        Self { radius, width }
    }
}

trait Actor {
    fn tile_pos(&self) -> &Pos<u32>;
    fn pos(&self) -> &Pos<f64>;
    fn update_pos(&mut self, pos: Pos<f64>);
    fn angle(&self) -> f64;
    fn light(&self) -> Option<&LightSpec>;
}

struct Player {
    tile_pos: Pos<u32>,
    pos: Pos<f64>,
    angle: f64,
    light: Option<LightSpec>,
}

impl Actor for Player {
    fn tile_pos(&self) -> &Pos<u32> {
        &self.tile_pos
    }

    fn pos(&self) -> &Pos<f64> {
        &self.pos
    }

    fn angle(&self) -> f64 {
        self.angle
    }

    fn light(&self) -> Option<&LightSpec> {
        self.light.as_ref()
    }

    fn update_pos(&mut self, pos: Pos<f64>) {
        self.tile_pos = to_tile_pos(&pos);
        self.pos = pos;
    }
}

fn to_tile_pos(pos: &Pos<f64>) -> Pos<u32> {
    Pos::new(pos.x.round() as u32, pos.y.round() as u32)
}

impl Player {
    fn new_with_light(pos: Pos<f64>, angle: f64, light: LightSpec) -> Self {
        Self {
            tile_pos: to_tile_pos(&pos),
            pos,
            angle,
            light: Some(light),
        }
    }
}

fn is_visible<A>(map: &Map, actor: &A, point: &Pos<u32>) -> bool
where
    A: Actor,
{
    if actor.tile_pos() == point {
        return true;
    }

    let Some(light) = actor.light() else {
        return false;
    };

    if distance(point, actor.tile_pos()) > light.radius {
        return false;
    }

    if !is_within_fov(actor, point, light) {
        return false;
    }

    if !cast_ray(actor.tile_pos(), point, map) {
        return false;
    }

    true
}

fn cast_ray(a: &Pos<u32>, b: &Pos<u32>, map: &Map) -> bool {
    let xdiff = a.x as i32 - b.x as i32;
    let xmul = match xdiff.cmp(&0) {
        Ordering::Less => 1.0,
        Ordering::Equal => 0.0,
        Ordering::Greater => -1.0,
    };
    let xdiff = xdiff.abs();
    let ydiff = a.y as i32 - b.y as i32;
    let ymul = match ydiff.cmp(&0) {
        Ordering::Less => 1.0,
        Ordering::Equal => 0.0,
        Ordering::Greater => -1.0,
    };
    let ydiff = ydiff.abs();
    let (xinc, yinc) = match ydiff.cmp(&xdiff) {
        Ordering::Less => {
            let xinc: f64 = 1.0 * xmul;
            let yinc = (ydiff as f64 / xdiff as f64) * ymul;
            (xinc, yinc)
        }
        Ordering::Equal => {
            let xinc: f64 = 1.0 * xmul;
            let yinc: f64 = 1.0 * ymul;
            (xinc, yinc)
        }
        Ordering::Greater => {
            let xinc = (xdiff as f64 / ydiff as f64) * xmul;
            let yinc: f64 = 1.0 * ymul;
            (xinc, yinc)
        }
    };
    let mut xcur = a.x as f64;
    let mut ycur = a.y as f64;
    while !(xcur.round() as usize == b.x as usize && ycur.round() as usize == b.y as usize) {
        let tile = map.at(xcur.round() as usize, ycur.round() as usize);
        if tile.obstructing() {
            return false;
        }
        xcur += xinc;
        ycur += yinc;
    }
    true
}

fn is_within_fov<A>(actor: &A, point: &Pos<u32>, light: &LightSpec) -> bool
where
    A: Actor,
{
    let dx = (actor.tile_pos().x as i32 - point.x as i32) as f64;
    let dy = (point.y as i32 - actor.tile_pos().y as i32) as f64;
    let atan = dy.atan2(dx) + consts::PI;
    let left = reduce_angle(actor.angle(), light.width / 2.0);
    let right = advance_angle(actor.angle(), light.width / 2.0);

    is_angle_between(atan, left, right)
}

fn calculate_visibility<A>(map: &Map, actor: &A, buffer: &mut Vec<bool>)
where
    A: Actor + Sync,
{
    debug_assert!(
        buffer.len() >= map.height() * map.width(),
        "visibility buffer to small, need {}, got {}",
        map.height() * map.width(),
        buffer.len()
    );

    map.tiles
        .par_iter()
        .enumerate()
        .map(|(index, _)| {
            let y = index / map.width();
            let x = index - y * map.width();
            if actor.tile_pos().x == x as u32 && actor.tile_pos().y == y as u32 {
                true
            } else {
                is_visible(map, actor, &Pos::new(x as u32, y as u32))
            }
        })
        .collect_into_vec(buffer)
}

fn print_map<A>(map: &Map, actor: &A, buffer: &mut Vec<bool>)
where
    A: Actor + Sync,
{
    let _ = execute!(io::stdout(), terminal::Clear(ClearType::All));

    calculate_visibility(map, actor, buffer);

    map.tiles
        .iter()
        .zip(buffer.iter())
        .enumerate()
        .map(|(index, (tile, visible))| {
            let y = index / map.width();
            let x = index - y * map.width();
            (
                index,
                if actor.tile_pos().x == x as u32 && actor.tile_pos().y == y as u32 {
                    '@'.black().on_white()
                } else if *visible {
                    match tile {
                        Tile::Wall(c) => c.on_red(),
                        Tile::Air => '*'.yellow(),
                    }
                } else {
                    match tile {
                        Tile::Wall(c) => c.dark_blue(),
                        Tile::Air => ' '.black(),
                    }
                },
            )
        })
        .for_each(|(index, c)| {
            print!("{}", c);
            if (index + 1) % map.width() == 0 {
                println!();
            }
        });
}

fn get_key() -> KeyCode {
    let _ = crossterm::terminal::enable_raw_mode();
    loop {
        if poll(Duration::from_millis(1000)).unwrap() {
            let event = read().unwrap();
            match event {
                Event::Key(ev) if ev.kind == KeyEventKind::Press => {
                    let _ = crossterm::terminal::disable_raw_mode();
                    return ev.code;
                }
                _ => (),
            }
        }
    }
}

fn advance_angle(mut a: f64, step: f64) -> f64 {
    a += step;
    if a > std::f64::consts::PI * 2.0 {
        a - std::f64::consts::PI * 2.0
    } else {
        a
    }
}

fn reduce_angle(mut a: f64, step: f64) -> f64 {
    a -= step;
    if a < 0.0 {
        a + std::f64::consts::PI * 2.0
    } else {
        a
    }
}

fn is_angle_between(a: f64, left: f64, right: f64) -> bool {
    if right < left {
        (a < right && a >= 0.0) || (a <= 2.0 * consts::PI && a > left)
    } else {
        left < a && right > a
    }
}

fn main() {
    let map = Map::from_file("maps/rust.txt");

    let mut player = Player::new_with_light(
        Pos::new(map.width() as f64 / 2.0, map.height() as f64 / 2.0),
        consts::PI,
        LightSpec::new(15.0, consts::PI / 4.0),
    );

    let mut visibility_buffer = vec![false; map.width() * map.height()];

    loop {
        calculate_visibility(&map, &player, &mut visibility_buffer);
        print_map(&map, &player, &mut visibility_buffer);
        let key = get_key();
        match key {
            KeyCode::Esc => break,
            KeyCode::Left => player.tile_pos.x -= 1,
            KeyCode::Right => player.tile_pos.x += 1,
            KeyCode::Up => player.tile_pos.y -= 1,
            KeyCode::Down => player.tile_pos.y += 1,
            KeyCode::PageUp => match player.light {
                None => (),
                Some(mut light) => {
                    light.radius += 0.5;
                    player.light = Some(light)
                }
            },
            KeyCode::PageDown => match player.light {
                None => (),
                Some(mut light) => {
                    light.radius -= 0.5;
                    player.light = Some(light)
                }
            },
            KeyCode::Home => player.angle = advance_angle(player.angle, consts::PI / 16.0),
            KeyCode::End => player.angle = reduce_angle(player.angle, consts::PI / 16.0),
            KeyCode::Insert => match player.light {
                None => (),
                Some(mut light) => {
                    light.width += 0.11;
                    player.light = Some(light)
                }
            },
            KeyCode::Delete => match player.light {
                None => (),
                Some(mut light) => {
                    light.width -= 0.11;
                    player.light = Some(light)
                }
            },
            KeyCode::Char('w') | KeyCode::Char('W') => {
                player.update_pos(Pos::new(
                    player.pos.x + player.angle.cos(),
                    player.pos.y - player.angle.sin(),
                ));
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                player.update_pos(Pos::new(
                    player.pos.x - player.angle.cos(),
                    player.pos.y + player.angle.sin(),
                ));
            }
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use crate::is_angle_between;

    #[test_case(5.0, 4.0, 6.0 => true; "inside")]
    #[test_case(5.0, 6.0, 7.0 => false; "outside - left")]
    #[test_case(5.0, 3.0, 4.0 => false; "outside - right")]
    #[test_case(0.0, 1.0, 2.0 => false; "outside - left - at 0")]
    #[test_case(0.0, -2.0, -1.0 => false; "outside - right - at 0")]
    #[test_case(0.0, -1.0, 1.0 => true; "inside - at 0")]
    #[test_case(1.0, 0.0, 2.0 => true; "inside - at 0 left border")]
    #[test_case(-1.0, -2.0, 0.0 => true; "inside - at 0 right border")]
    #[test_case(6.28, 6.18, 0.1 => true; "inside - at PI looping")]
    fn between(a: f64, left: f64, right: f64) -> bool {
        is_angle_between(a, left, right)
    }
}
