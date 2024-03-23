use std::{borrow::Cow, fs, path::Path};

use anyhow::Result;
use serde::Deserialize;

fn main() -> Result<()> {
    let bytes = include_bytes!("../samples/teapot.bmp");
    let image = BMP::new(bytes)?;
    let (width, height) = image.size();
    let mut rotated = image.clone();
    for y in 0..height {
        for x in 0..width {
            rotated
                .set((x, y), image.get((y, width - x - 1)).unwrap())
                .unwrap();
        }
    }
    rotated.save("rotated.bmp").unwrap();
    println!("rotated.bmp written");

    Ok(())
}

#[derive(Deserialize, Debug, Default, Clone)]
struct BMPInfo {
    header: Header,
    dib: DIB,
}

#[derive(Deserialize, Debug, Default, Clone)]
struct Header {
    _signature: u16,
    _size: u32,
    _reserved: u32,
    offset: u32,
}

#[derive(Deserialize, Debug, Default, Clone)]
struct DIB {
    _header_size: u32,
    width: i32,
    height: i32,
    _planes: u16,
    bpp: u16,
    _compression: u32,
    _size: u32,
    _hres: i32,
    _vres: i32,
    _colors: u32,
    _important_colors: u32,
    _rmask: u32,
    _gmask: u32,
    _bmask: u32,
    _amask: u32,
    _cs_type: u32,
    _cs_endpoints: [u32; 9],
    _rgamma: u32,
    _ggamma: u32,
    _bgamma: u32,
    _intent: u32,
    _icc: u32,
    _icc_size: u32,
    _reserved: u32,
}

#[derive(Clone)]
struct BMP<'a> {
    info: BMPInfo,
    data: Cow<'a, [u8]>,
}

impl<'a> BMP<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self> {
        let info: BMPInfo = bincode::deserialize(data)?;
        // This implementation assumes 24 bits per pixel.
        assert_eq!(info.dib.bpp, 24);
        Ok(Self {
            info,
            data: Cow::from(data),
        })
    }

    pub fn size(&self) -> (usize, usize) {
        let dib = &self.info.dib;
        (dib.width as usize, dib.height as usize)
    }

    pub fn get(&self, (x, y): (usize, usize)) -> Option<[u8; 3]> {
        let ofs = self.pixel_index((x, y))?;
        // Read the bytes in little endian order.
        Some([
            *self.data.get(ofs + 2)?,
            *self.data.get(ofs + 1)?,
            *self.data.get(ofs)?,
        ])
    }

    fn pixel_index(&self, (x, y): (usize, usize)) -> Option<usize> {
        let (width, height) = self.size();
        if x >= width || y >= height {
            return None;
        }
        let bytes_per_pixel = self.info.dib.bpp as usize / 8;
        let bytes_per_row = bytes_per_pixel * width;
        let row = height - y - 1;
        Some(self.info.header.offset as usize + bytes_per_row * row + bytes_per_pixel * x)
    }

    pub fn set(&mut self, (x, y): (usize, usize), value: [u8; 3]) -> Option<()> {
        let ofs = self.pixel_index((x, y))?;
        *self.data.to_mut().get_mut(ofs + 2)? = value[0];
        *self.data.to_mut().get_mut(ofs + 1)? = value[1];
        *self.data.to_mut().get_mut(ofs)? = value[2];
        Some(())
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        fs::write(path, self.data.as_ref())?;
        Ok(())
    }
}
