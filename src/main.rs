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

    fn _height(&self) -> usize {
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

fn distance(a: &Pos, b: &Pos) -> f64 {
    let dx = a.x as i32 - b.x as i32;
    let dy = a.y as i32 - b.y as i32;

    let dxs = dx * dx;
    let dys = dy * dy;

    ((dxs + dys) as f64).sqrt()
}

#[derive(PartialEq)]
struct Pos {
    x: u32,
    y: u32,
}

impl Pos {
    fn new(x: u32, y: u32) -> Self {
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
    fn pos(&self) -> &Pos;
    fn angle(&self) -> f64;
    fn light(&self) -> Option<&LightSpec>;
}

struct Player {
    pos: Pos,
    angle: f64,
    light: Option<LightSpec>,
}

impl Actor for Player {
    fn pos(&self) -> &Pos {
        &self.pos
    }

    fn angle(&self) -> f64 {
        self.angle
    }

    fn light(&self) -> Option<&LightSpec> {
        self.light.as_ref()
    }
}

impl Player {
    fn _new(pos: Pos, angle: f64) -> Self {
        Self {
            pos,
            angle,
            light: None,
        }
    }

    fn new_with_light(pos: Pos, angle: f64, light: LightSpec) -> Self {
        Self {
            pos,
            angle,
            light: Some(light),
        }
    }
}

fn is_visible<A>(map: &Map, actor: &A, point: &Pos) -> bool
where
    A: Actor,
{
    if actor.pos() == point {
        return true;
    }

    let Some(light) = actor.light() else {
        return false;
    };

    if distance(point, actor.pos()) > light.radius {
        return false;
    }

    if !is_within_fov(actor, point, light) {
        return false;
    }

    let xdiff = actor.pos().x as i32 - point.x as i32;
    let xmul = match xdiff.cmp(&0) {
        Ordering::Less => 1.0,
        Ordering::Equal => 0.0,
        Ordering::Greater => -1.0,
    };
    let xdiff = xdiff.abs();

    let ydiff = actor.pos().y as i32 - point.y as i32;
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

    let mut xcur = actor.pos().x as f64;
    let mut ycur = actor.pos().y as f64;

    loop {
        let tile = map.at(xcur.round() as usize, ycur.round() as usize);
        if tile.obstructing() {
            return false;
        }
        xcur += xinc;
        ycur += yinc;

        if xcur.round() as usize == point.x as usize && ycur.round() as usize == point.y as usize {
            return true;
        }
    }
}

fn is_within_fov<A>(actor: &A, point: &Pos, light: &LightSpec) -> bool
where
    A: Actor,
{
    let dx = (actor.pos().x as i32 - point.x as i32) as f64;
    let dy = (point.y as i32 - actor.pos().y as i32) as f64;
    let atan = dy.atan2(dx) + consts::PI;
    let left = reduce_angle(actor.angle(), light.width / 2.0);
    let right = advance_angle(actor.angle(), light.width / 2.0);

    is_angle_between(atan, left, right)
}

fn calculate_visibility<A>(map: &Map, actor: &A, buffer: &mut Vec<bool>)
where
    A: Actor + Sync,
{
    map.tiles
        .par_iter()
        .enumerate()
        .map(|(index, _)| {
            let y = index / map.width();
            let x = index - y * map.width();
            if actor.pos().x == x as u32 && actor.pos().y == y as u32 {
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
                if actor.pos().x == x as u32 && actor.pos().y == y as u32 {
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
        Pos::new((map.width() / 2) as u32, (map.height() / 2) as u32),
        consts::PI,
        LightSpec::new(15.0, consts::PI / 4.0),
    );

    let map = Map::from_file("maps/rust.txt");

    let mut visibility_buffer = vec![false; WIDTH * HEIGHT];

    loop {
        calculate_visibility(&map, &player, &mut visibility_buffer);
        print_map(&map, &player, &mut visibility_buffer);
        let key = get_key();
        match key {
            KeyCode::Esc => break,
            KeyCode::Left => player.pos.x -= 1,
            KeyCode::Right => player.pos.x += 1,
            KeyCode::Up => player.pos.y -= 1,
            KeyCode::Down => player.pos.y += 1,
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
