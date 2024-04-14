use core::fmt;
use std::{cmp::Ordering, io};

use crossterm::{
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
    let (xinc, yinc) = if x < px && y < py {
        // Top-left corner
        let xdiff = px - x;
        let ydiff = py - y;
        match ydiff.cmp(&xdiff) {
            Ordering::Less => {
                let xinc: f64 = -1.0;
                let yinc = -(ydiff as f64 / xdiff as f64);
                (xinc, yinc)
            }
            Ordering::Equal => {
                let yinc: f64 = -1.0;
                let xinc: f64 = -1.0;
                (xinc, yinc)
            }
            Ordering::Greater => {
                let yinc: f64 = -1.0;
                let xinc = -(xdiff as f64 / ydiff as f64);
                (xinc, yinc)
            }
        }
    } else {
        return false;
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
    for y in 0..map.height() {
        for x in 0..map.width() {
            let tile = map.at(x, y);
            let c = if px == x && py == y {
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
            };
            print!("{}", c);
        }
        println!()
    }
}

fn main() {
    let px = 15;
    let py = 15;

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

    print_map(&map, px, py)
}
