use core::fmt;
use std::{cmp::Ordering, io, time::Duration};

use crossterm::{
    event::{poll, read, Event, KeyCode, KeyEventKind},
    execute,
    style::Stylize,
    terminal::{self, ClearType},
};

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

    fn height(&self) -> usize {
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

fn is_visible(x: usize, y: usize, px: usize, py: usize, map: &Map) -> bool {
    if x == px && y == px {
        return true;
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
        xcur = xcur + xinc;
        ycur = ycur + yinc;

        if xcur.round() as usize == x && ycur.round() as usize == y {
            return true;
        }
    }
}

fn print_map(map: &Map, px: usize, py: usize) {
    let _ = execute!(io::stdout(), terminal::Clear(ClearType::All));

    map.tiles
        .iter()
        .enumerate()
        .map(|(index, tile)| {
            let y = index / map.width();
            let x = index - y * map.width();
            (
                index,
                if px == x && py == y {
                    "@".white()
                } else {
                    if is_visible(x, y, px, py, map) {
                        match tile {
                            Tile::Wall => "#".blue(),
                            Tile::Air => ".".yellow(),
                        }
                    } else {
                        "-".dark_blue()
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

fn main() {
    let mut px = 15;
    let mut py = 15;

    let mut map = Map::new(30, 20);

    // Square in top left corner
    map.set_at(3 + 4, 3, Tile::Wall);
    map.set_at(4 + 4, 3, Tile::Wall);
    map.set_at(5 + 4, 3, Tile::Wall);
    map.set_at(6 + 4, 3, Tile::Wall);
    map.set_at(3 + 4, 4, Tile::Wall);
    map.set_at(4 + 4, 4, Tile::Wall);
    map.set_at(5 + 4, 4, Tile::Wall);
    map.set_at(6 + 4, 4, Tile::Wall);
    map.set_at(3 + 4, 5, Tile::Wall);
    map.set_at(4 + 4, 5, Tile::Wall);
    map.set_at(5 + 4, 5, Tile::Wall);
    map.set_at(6 + 4, 5, Tile::Wall);

    // Another square in top left corner
    map.set_at(2, 10, Tile::Wall);
    map.set_at(2, 11, Tile::Wall);
    map.set_at(2, 12, Tile::Wall);

    map.set_at(10, 10, Tile::Wall);

    loop {
        print_map(&map, px, py);
        let key = get_key();
        match key {
            KeyCode::Esc => break,
            KeyCode::Left => px = px - 1,
            KeyCode::Right => px = px + 1,
            KeyCode::Up => py = py - 1,
            KeyCode::Down => py = py + 1,
            _ => (),
        }
    }
}
