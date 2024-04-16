use core::fmt;
use std::{cmp::Ordering, io, time::Duration};

use crossterm::{
    event::{poll, read, Event, KeyCode, KeyEventKind},
    execute,
    style::Stylize,
    terminal::{self, ClearType},
};
use rand::Rng;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

#[derive(Clone)]
enum Tile {
    Wall,
    Air,
}

impl Tile {
    fn obstructing(&self) -> bool {
        match self {
            Tile::Wall => true,
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

    fn set_at(&mut self, x: usize, y: usize, tile: Tile) {
        self.tiles[y * self.width + x] = tile
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
                Tile::Wall => "#".blue(),
                Tile::Air => ".".yellow(),
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

fn is_visible(x: usize, y: usize, px: usize, py: usize, radius: Option<f64>, map: &Map) -> bool {
    if x == px && y == py {
        return true;
    }
    if let Some(radius) = radius {
        if distance(x as i32, y as i32, px as i32, py as i32) > radius {
            return false;
        }
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

fn calculate_visibility(map: &Map, px: usize, py: usize, radius: f64) -> Vec<bool> {
    map.tiles
        .par_iter()
        .enumerate()
        .map(|(index, _)| {
            let y = index / map.width();
            let x = index - y * map.width();
            if px == x && py == y {
                true
            } else {
                is_visible(x, y, px, py, Some(radius), map)
            }
        })
        .collect()
}

fn print_map(map: &Map, px: usize, py: usize, radius: f64) {
    let _ = execute!(io::stdout(), terminal::Clear(ClearType::All));

    let visibility_map = calculate_visibility(map, px, py, radius);

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
                    "@".white()
                } else if *visible {
                    match tile {
                        Tile::Wall => " ".on_red(),
                        Tile::Air => "*".yellow(),
                    }
                } else {
                    ".".dark_blue()
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

fn main() {
    const WIDTH: usize = 128;
    const HEIGHT: usize = 64;

    let mut rng = rand::thread_rng();

    let mut px = WIDTH / 2;
    let mut py = HEIGHT / 2;
    let mut radius = 5.0;

    let mut map = Map::new(WIDTH, HEIGHT);
    let wall_count = rng.gen_range(10..WIDTH * HEIGHT / 4);

    (0..wall_count).for_each(|_| {
        let x = rng.gen_range(0..WIDTH);
        let y = rng.gen_range(0..HEIGHT);
        map.set_at(x, y, Tile::Wall);
    });

    loop {
        calculate_visibility(&map, px, py, radius);
        print_map(&map, px, py, radius);
        let key = get_key();
        match key {
            KeyCode::Esc => break,
            KeyCode::Left => px -= 1,
            KeyCode::Right => px += 1,
            KeyCode::Up => py -= 1,
            KeyCode::Down => py += 1,
            KeyCode::PageUp => radius += 0.5,
            KeyCode::PageDown => radius -= 0.5,
            _ => (),
        }
    }
}
