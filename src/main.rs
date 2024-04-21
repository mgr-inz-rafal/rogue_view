use core::fmt;
use std::{
    cmp::Ordering,
    f64::consts,
    fs::File,
    io::{self, stdout, BufRead, BufReader, Write},
    path::Path,
    time::Duration,
};

use crossterm::{
    cursor::{self, MoveTo},
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
        for line in lines {
            for c in line.chars() {
                let tile = match c {
                    ' ' => Tile::Air,
                    c => Tile::Wall(c),
                };
                map.tiles.push(tile);
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

fn distance(x1: i32, y1: i32, x2: i32, y2: i32) -> f64 {
    let dx = x1 - x2;
    let dy = y1 - y2;

    let dxs = dx.pow(2);
    let dys = dy.pow(2);

    ((dxs + dys) as f64).sqrt()
}

// TODO: Split into various checks. Execute from the cheapest to the most expensive.
fn is_visible(
    x: usize,
    y: usize,
    px: usize,
    py: usize,
    radius: Option<f64>,
    map: &Map,
    pa: f64,
    pfov: f64,
) -> bool {
    if x == px && y == py {
        return true;
    }

    if let Some(radius) = radius {
        if distance(x as i32, y as i32, px as i32, py as i32) > radius {
            return false;
        }
    }

    // TODO: Use FoV.
    let dx = (px as i32 - x as i32) as f64;
    let dy = (y as i32 - py as i32) as f64;
    let atan = dy.atan2(dx) + consts::PI;
    let left = reduce_angle(pa, pfov / 2.0);
    let right = advance_angle(pa, pfov / 2.0);

    if !is_between(atan, left, right) {
        return false;
    }

    let xdiff = px as i32 - x as i32;
    let xmul = match xdiff.cmp(&0) {
        Ordering::Less => 1.0,
        Ordering::Equal => 0.0,
        Ordering::Greater => -1.0,
    };
    let xdiff = xdiff.abs();

    let ydiff = py as i32 - y as i32;
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

    let mut xcur = px as f64;
    let mut ycur = py as f64;

    loop {
        let tile = map.at(xcur.round() as usize, ycur.round() as usize);
        if tile.obstructing() {
            return false;
        }
        xcur += xinc;
        ycur += yinc;

        if xcur.round() as usize == x && ycur.round() as usize == y {
            return true;
        }
    }
}

fn calculate_visibility(
    map: &Map,
    px: usize,
    py: usize,
    radius: f64,
    pa: f64,
    pfov: f64,
) -> Vec<bool> {
    map.tiles
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let y = index / map.width();
            let x = index - y * map.width();
            if px == x && py == y {
                true
            } else {
                is_visible(x, y, px, py, Some(radius), map, pa, pfov)
            }
        })
        .collect()
}

fn print_map(map: &Map, px: usize, py: usize, radius: f64, pa: f64, pfov: f64) {
    let _ = execute!(io::stdout(), terminal::Clear(ClearType::All));

    let visibility_map = calculate_visibility(map, px, py, radius, pa, pfov);

    map.tiles
        .iter()
        .zip(visibility_map.iter())
        .enumerate()
        .map(|(index, (tile, visible))| {
            let y = index / map.width();
            let x = index - y * map.width();
            (
                index,
                if px == x && py == y {
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

fn to_deg(radians: f64) -> f64 {
    radians * (180.0 / std::f64::consts::PI)
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

fn is_between(a: f64, left: f64, right: f64) -> bool {
    // dbg!(a);
    // dbg!(left);
    // dbg!(right);
    // panic!();
    if right < left {
        (a < right && a >= 0.0) || (a <= 360.0 && a > left)
    } else {
        left < a && right > a
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use crate::is_between;

    #[test_case(90.0, 45.0, 135.0 => true)]
    #[test_case(0.0, 315.0, 45.0 => true)]
    #[test_case(45.0, 0.0, 90.0 => true)]
    #[test_case(315.0, 270.0, 360.0 => true)]
    #[test_case(315.0, 270.0, 0.0 => true)]
    fn between(a: f64, left: f64, right: f64) -> bool {
        is_between(a, left, right)
    }
}

fn main() {
    const WIDTH: usize = 128;
    const HEIGHT: usize = 64;

    let mut pfov = consts::PI / 4.0;
    let mut px = WIDTH / 2;
    let mut py = HEIGHT / 2 - 10;
    //let mut py = 2;
    let mut radius = 15.0;

    // let mut map = Map::new(WIDTH, HEIGHT);
    // let wall_count = rng.gen_range(10..WIDTH * HEIGHT / 4);
    // (0..wall_count).for_each(|_| {
    // let x = rng.gen_range(0..WIDTH);
    // let y = rng.gen_range(0..HEIGHT);
    // map.set_at(x, y, Tile::Wall);
    // });

    // let mut px = 10;
    // let mut py = 10;

    // let mut i = 0;
    // let xx = vec![8, 9, 10, 11, 12, 12, 12, 12, 12, 11, 10, 9, 8, 8, 8, 8];
    // let yy = vec![8, 8, 8, 8, 8, 9, 10, 11, 12, 12, 12, 12, 12, 11, 10, 9];

    let mut pa = consts::PI;
    let mut left = pa - consts::PI / 4.0;
    let mut right = pa + consts::PI / 4.0;

    let mut stdout = stdout();
    /*
         loop {
            let x = xx[i];
            let y = yy[i];
            let _ = execute!(io::stdout(), terminal::Clear(ClearType::All));
            let _ = execute!(stdout, MoveTo(px, py));
            println!("@");
            let _ = execute!(stdout, MoveTo(x, y));
            println!("*");

            let _ = execute!(stdout, MoveTo(0, 15));

            let dx = (x as i32 - px as i32) as f64;
            let dy = (y as i32 - py as i32) as f64;
            let atan = dy.atan2(dx);
            println!("atan={atan} ({})", to_deg(atan));
            let matan = atan + std::f64::consts::PI;
            println!("matan={matan} ({})", to_deg(matan));
            println!("pa={pa} ({})", to_deg(pa));

            println!("left={left} ({})", to_deg(left));
            println!("right={right} ({})", to_deg(right));

            let key = get_key();
            match key {
                KeyCode::Esc => break,
                KeyCode::PageUp => {
                    pa = advance_angle(pa, std::f64::consts::PI / 8.0);
                    left = advance_angle(left, std::f64::consts::PI / 8.0);
                    right = advance_angle(right, std::f64::consts::PI / 8.0);
                }
                KeyCode::PageDown => {
                    pa = reduce_angle(pa, std::f64::consts::PI / 8.0);
                    left = reduce_angle(left, std::f64::consts::PI / 8.0);
                    right = reduce_angle(right, std::f64::consts::PI / 8.0);
                }
                KeyCode::Char(' ') => {
                    i += 1;
                    if i == xx.len() {
                        i = 0;
                    }
                }
                _ => (),
            }
        }
    */

    let map = Map::from_file("maps/rust.txt");

    loop {
        calculate_visibility(&map, px, py, radius, pa, pfov);
        print_map(&map, px, py, radius, pa, pfov);
        let key = get_key();
        match key {
            KeyCode::Esc => break,
            KeyCode::Left => px -= 1,
            KeyCode::Right => px += 1,
            KeyCode::Up => py -= 1,
            KeyCode::Down => py += 1,
            KeyCode::PageUp => radius += 0.5,
            KeyCode::PageDown => radius -= 0.5,
            KeyCode::Home => pa = advance_angle(pa, consts::PI / 16.0),
            KeyCode::End => pa = reduce_angle(pa, consts::PI / 16.0),
            KeyCode::Insert => pfov -= 0.11,
            KeyCode::Delete => pfov += 0.11,
            _ => (),
        }
    }
}
