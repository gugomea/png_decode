#![allow(non_snake_case)]
use std::fs;

use crate::zlib;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChunkType {
    IHDR, //image header, which is the first chunk in a PNG datastream.
    //PLTE, //palette table associated with indexed PNG images.
    IDAT, //image data chunks.
    IEND, //image trailer, which is the last chunk in a PNG datastream.
    AncyllaryChunk, //the ones we will ignore for simplicity purposes.
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    pub typ: ChunkType,
    pub data: Vec<u8>,
}

//      VALID IMAGE TYPE - COLOUR TYPE
// ┌───────────────────────┬──────────────┐
// │    PNG image type     │ Colour type  │
// ├───────────────────────┼──────────────┤
// │ Greyscale             │      0       │ [0-255]
// │ Truecolour            │      2       │ [R, G, B] <- each one between 0-255
// │ Indexed-colour        │      3       │ <- Won´t be supported.
// │ Greyscale with alpha  │      4       │ [[0-255], A]
// │ Truecolour with alpha │      6       │ [R, G, B, A] <- each one between 0-255
// └───────────────────────┴──────────────┘
// images of more than 8 bit depth will be truncated

impl Chunk {
    pub fn from_slice(bytes: &[u8]) -> Chunk {
        use ChunkType::*;
        let length = u32::from_be_bytes(bytes[..4].try_into().expect("Valid length"));
        let typ = match &bytes[4..8] {
            b"IHDR" => IHDR,
            //b"PLTE" => PLTE,
            b"IDAT" => IDAT,
            b"IEND" => IEND,
            _ => AncyllaryChunk,
        };
        let data = bytes[8.. 8 + length as usize].to_vec();
        // TODO: CHECKSUM
        Chunk { typ, data }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageHeader {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub color_type: ColorType,
}

impl ImageHeader {
    pub fn from_chunk(header: Chunk) -> Self{
        if header.typ != ChunkType::IHDR {
            panic!("ChunkType provided: {:?}, expected IHDR", header.typ);
        }
        let data = header.data;
        let width = u32::from_be_bytes(data[..4].try_into().expect("Valid length"));
        let height = u32::from_be_bytes(data[4..8].try_into().expect("Valid length"));
        let bit_depth = data[8];
        use ColorType::*;
        let color_type = match data[9] {
            0 => Greyscale,
            2 => Truecolour,
            4 => GreyscaleA,
            6 => TruecolourA,
            invalid => panic!("Invalid Colour type: {}", invalid),
        };
        Self {
            width,
            height,
            bit_depth,
            color_type,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorType {
    Greyscale,
    Truecolour,
    GreyscaleA,
    TruecolourA,
}

pub struct PNGImage {
    pub header: ImageHeader,
    pub bytes_per_pixel: u8,
    pub data: Vec<u8>,
}

impl PNGImage {
    pub fn from_path(path: &str) -> Self {
        let f = fs::read(path).unwrap();

        match &f[..8] {
            b"\x89PNG\r\n\x1a\n" => println!("Header Ok."),
            invalido => panic!("invalid header : {:x?}\n", invalido),
        }
        let mut chunks = vec![];
        let mut i = 8;
        while i != f.len(){
            let chunk = Chunk::from_slice(&f[i..]);
            i += chunk.data.len() + 12; // 12 = (length) + (type) + (CRC) = 4 + 4 + 4
            chunks.push(chunk);
        }
        let image = PNGImage::from_chunks(chunks);

        return image;
    }
    pub fn from_chunks(chunks: Vec<Chunk>) -> Self {
        use ChunkType::IDAT;
        let header = ImageHeader::from_chunk(chunks[0].clone());

        let bytes_per_pixel = match header.color_type {
            ColorType::Greyscale => 1,
            ColorType::GreyscaleA => 2,
            ColorType::Truecolour => 3,
            ColorType::TruecolourA => 4,
        } * ((header.bit_depth + 7) / 8);
        let image = Self { header, bytes_per_pixel, data: vec![] };

        let mut compressed_data = vec![];
        for mut chunk in chunks {
            if chunk.typ == IDAT {
                compressed_data.append(&mut chunk.data);
            }
        }
        println!("INFLATING..");
        let decompressed_data = zlib::inflate(&compressed_data);
        //let decompressed_data = miniz_oxide::inflate::decompress_to_vec_zlib(&compressed_data).unwrap();

        //println!("UNFILTERING..");
        let data = image.unfilter(decompressed_data);

        Self { header, bytes_per_pixel, data }
    }

    fn PaethPredictor(a: i32, b: i32, c: i32) -> i32 {
        let p = a + b - c;
        let pa = i32::abs(p - a);
        let pb = i32::abs(p - b);
        let pc = i32::abs(p - c);
        if pa <= pb && pa <= pc {
            return a
        }
        else if pb <= pc {
            return b
        } else {
            return c
        }
    }

    fn unfilter(self, decompressed_data: Vec<u8>) -> Vec<u8> {
        let rows = self.header.height as usize;
        let cols = self.header.width as usize * self.bytes_per_pixel as usize;

        //┌───┬──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
        //│ x │	the byte being filtered;					                                                                                                             │
        //│ a │	the byte corresponding to x in the pixel immediately before the pixel containing x (or the byte immediately before x, when the bit depth is less than 8);│
        //│ b │	the byte corresponding to x in the previous scanline;                                                                                                    │
        //│ c │	the byte corresponding to b in the pixel immediately before the pixel containing b (or the byte immediately before b, when the bit depth is less than 8).│
        //└───┴──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
        let mut data = Vec::with_capacity(rows * cols);//vec![0u8; ];
        for i in 0..rows {
            let typ = decompressed_data[i * (cols + 1)];
            for j in 0..cols {
                let x = decompressed_data[i * (cols + 1) + j + 1] as u16;
                let xi = i * cols + j;
                let (left, up) = (j >= self.bytes_per_pixel as usize,  i  > 0);
                let a = match left {
                    true => data[xi - self.bytes_per_pixel as usize] as u16,
                    false => 0,
                };

                let b = match up {
                    true =>  data[xi - cols] as u16,
                    false => 0,
                    
                };
                let c = match left && up {
                    true => data[xi - cols - self.bytes_per_pixel as usize] as u16,
                    false => 0,
                };
                /*data[xi] = */data.push(match typ {
                    0 => x as u8,
                    1 => ((x + a) & 255) as u8,
                    2 => ((x + b) & 255) as u8,
                    3 => ((x + ((a as usize + b as usize) / 2) as u16 ) & 255) as u8,
                    4 => ((x + (Self::PaethPredictor(a as i32, b as i32 , c as i32)) as u16) & 255) as u8, //x.wrapping_add(),
                    invalid => panic!("invalid filter method, type: {}", invalid),
                });
            }
        }
        return data;
    }
}
