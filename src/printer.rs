use std::{io::{stdout, Write}, ops::{Div, Mul, Add}};

use crossterm::{style::{Color, PrintStyledContent, Stylize, Print}, queue};

use crate::png::*;
// greyscale -> [grey, grey, grey]
//true color -> [r, b ,g]
// ture color A -> [r, g, b] ignore alpha
// greyscale A => [grey, grey, gey] -> ignore alpha
/* 
The most accurate scaling is achieved by the linear equation

output = floor((input * MAXOUTSAMPLE / MAXINSAMPLE) + 0.5)

where

MAXINSAMPLE = (2 ^ sampledepth) - 1
MAXOUTSAMPLE = (2 ^ desired_sampledepth) - 1

Cause we are going to print them to terminal we dont care that much about precission
so the approximate formula would be input / 2 ^ (real_bit_depth - 8) which would result
in shifting that many bites to right, or just taking first chunk from raw bytes cause we are
in big endian.
*/
pub struct ImgData {
    h: u32,
    w: u32,
    pixels: Vec<Pixel>,
}

impl ImgData {
    pub fn new(w: u32, h: u32, pixels: &[u8], typ: ColorType, bit_depth: u8) -> ImgData {

        let pixels = pixels.chunks((bit_depth as usize + 7) / 8usize).map(|x| x[0]).collect::<Vec<u8>>();
        
        let pixels: Vec<Pixel> = match typ {
            ColorType::Greyscale => pixels.chunks(1).map(|x| Pixel::from_u8(x[0], x[0], x[0])).collect(),
            ColorType::GreyscaleA => pixels.chunks(2).map(|x| Pixel::from_u8(x[0], x[0], x[0])).collect(),
            ColorType::Truecolour => pixels.chunks(3).map(|x| Pixel::from_u8(x[0], x[1], x[2])).collect(),
            ColorType::TruecolourA => pixels.chunks(4).map(|x| Pixel::from_u8(x[0], x[1], x[2])).collect(),
        };

        return ImgData { h, w, pixels };
    }

}

pub fn print(img: ImgData) {
    println!("PRINTING...");
    let mut stdout = stdout();

    let n = img.h as usize;
    let w = img.w as usize;

    for dx in 0..(n/2) {
        for col in 0..w {
            let row = 2 * dx;
            let upper_pixel = img.pixels[row * w + col];
            let lower_pixel = img.pixels[(row + 1) * w + col];
            let color_upper_pixel = Color::Rgb { r: upper_pixel.r as u8, g: upper_pixel.g as u8, b: upper_pixel.b as u8 };
            let color_lower_pixel = Color::Rgb { r: lower_pixel.r as u8, g: lower_pixel.g as u8, b: lower_pixel.b as u8 };
            queue!(stdout, PrintStyledContent("\u{2580}".with(color_upper_pixel).on(color_lower_pixel))).expect("msg");
        }
        queue!(stdout, Print("\n")).expect("msg");
    }

    if n % 2 != 0 {
        for col in 0..w {
            let px = img.pixels[(n - 1) * w + col];
            let color_px = Color::Rgb { r: px.r as u8, g: px.g as u8, b: px.b as u8 };
            queue!(stdout, PrintStyledContent("\u{2580}".with(color_px))).expect("msg");
        }
        queue!(stdout, Print("\n")).expect("msg");
    }

    stdout.flush().expect("msg");
}

pub fn resize(mut img: ImgData, t_width: f32) -> ImgData {
    println!("RESIZING...");
    let (w, h) = (img.w as f32, img.h as f32);
    let w2 = t_width as usize;
    let m_x = (h / w) * t_width;
    let h2 = m_x as usize;
    //println!("current width: {}", w2);
    //println!("current height: {}", h2);
    let (mult_w, mult_h) = (w / w2 as f32, h / h2 as f32);
    //println!("1 current pixel = {} original pixels in width", mult_w);
    //println!("1 current pixel = {} original pixels in height", mult_h);

    //println!("PROCESSING PIXELS");
    let mut new_pixels = Vec::with_capacity(h2 * w2);
    for i in 0..h2 {
        for j in 0..w2 {
            let px = process_rows(i as f32, j as f32, mult_h, &img.pixels, mult_w, w as usize);
            new_pixels.push(px);
        }
    }

    img.w = w2 as u32;
    img.h = h2 as u32;
    img.pixels = new_pixels;

    return img;
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Pixel {
    r: usize,
    g: usize,
    b: usize,
}
impl Pixel {
    fn from_u8(r: u8, g: u8, b: u8) -> Pixel {
        return Pixel {
            r: r as usize,
            g: g as usize,
            b: b as usize,
        };
    }
}

impl Add for Pixel {
    type Output = Pixel;
    fn add(self, rhs: Pixel) -> Self::Output {
        return Pixel { r: self.r + rhs.r, g: self.g + rhs.g, b: self.b + rhs.b, };
    }
}

impl Mul<usize> for Pixel {
    type Output = Pixel;
    fn mul(self, rhs: usize) -> Self::Output {
        return Pixel { r: self.r * rhs, g: self.g * rhs, b: self.b * rhs, };
    }
}

impl Div<f32> for Pixel {
    type Output = Pixel;
    fn div(self, rhs: f32) -> Self::Output {
        return Pixel { r: (self.r as f32 / rhs) as usize, g: (self.g as f32 / rhs) as usize, b: (self.b as f32 / rhs) as usize, };
    }
}

fn process_pixel(n_col: f32, mult: f32, data: &[Pixel]) -> Pixel {
    //println!("+=======PROCESSING PIXEL===========+");
    let start = two_digits(mult * n_col);
    let end = two_digits(mult * (n_col + 1.0));
    let t_rate = start.ceil() - start + end - end.floor();
    let f_rate = (((start.ceil() - start) / t_rate) * 10.0).floor() as usize;
    let l_rate = (((end - end.floor()) / t_rate) * 10.0).floor() as usize;

    //println!("first: {} ~= {}", start.ceil() - start, f_rate);
    //println!("last: {} ~= {}", end - end.floor(), l_rate);
    let count = end as usize - start.ceil() as usize;
    let mut px_total = Pixel::default();

    //println!("principio {:?}", data[start as usize]);
    for px in start.ceil() as usize..end as usize {
        px_total = px_total + data[px] * 10;
    }
    //println!("fin {:?}", data[end as usize]);

    if (end as usize) < data.len() {
        px_total = px_total + data[start as usize] * (f_rate) + data[end as usize] * (l_rate);
    }else {
        px_total = px_total + data[start as usize] * (f_rate) ;//+ data[end as usize] * (l_rate);
    }

    return px_total / (10*count + f_rate + l_rate) as f32;
}

fn process_rows(n_row: f32, n_col: f32, mult: f32, data: &[Pixel], mult2: f32, pixels_per_row: usize) -> Pixel {
    let start = two_digits(mult * n_row);
    let end = two_digits(mult * (n_row + 1.0));
    let t_rate = start.ceil() - start + end - end.floor();
    let f_rate = (((start.ceil() - start) / t_rate) * 10.0).floor() as usize;
    let l_rate = (((end - end.floor()) / t_rate) * 10.0).floor() as usize;

    let count = end as usize - start.ceil() as usize;
    let mut px_total = Pixel::default();

    for _row in start.ceil() as usize..end as usize {
        px_total = px_total + process_pixel(n_col, mult2, &data[pixels_per_row*_row as usize..]) * 10;
    }

    if data[pixels_per_row*end as usize..].len() > 0{
        px_total = px_total + process_pixel(n_col, mult2, &data[pixels_per_row*start as usize..]) * (f_rate) 
                   + process_pixel(n_col, mult2, &data[pixels_per_row*end as usize..]) * (l_rate);
    }else {
        px_total = px_total + process_pixel(n_col, mult2, &data[pixels_per_row*start as usize..]) * (f_rate);
    }
    
    return px_total / (10*count + f_rate + l_rate) as f32;
}

fn two_digits(n: f32) -> f32 {
    return (n * 10.0).trunc()/ 10.0;
}
//#[cfg(test)]
//mod tests {
//    use crate::PNGImage;
//
//    use super::resize;
//
//    #[test]
//    fn test1() {
//        let img = PNGImage {
//            bytes_per_pixel: 3,
//            data: vec![],
//            header: crate::ImageHeader { width: 600, height: 700, bit_depth: 8, color_type: crate::ColorType::Truecolour }
//        };
//        resize(&img, 0.0, 100.0);
//    }
//}
