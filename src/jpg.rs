pub fn print_file(path: &str) {
    let file = std::fs::read(path).expect("msg");
    println!("{:x?}", &file[..500]);
    let mut data = BitStream::new(&file);
    //println!("{:x?}", &file);
    let soi =u16::from_be_bytes([data.next_byte(), data.next_byte()]);
    match soi {
        0xFFD8 => println!("SOI OK: {:x?}", soi),
        invalid => println!("INVALID SOI: {:x?}", invalid),
    }

    let mut prev = data.next_byte();
    let mut curr = data.next_byte();
    println!("previus current: {:x}, {:x}", prev, curr);

    let mut img = JpgImg::new();

    while (prev, curr) != (0xFF, 0xD9) {
        if prev != 0xFF {
            panic!("Expected marker(0xFF) got: {}", prev);
        }

        match curr {
            0xE0..=0xEF => process_appn(&mut data),//aplication especific data (we don´t care)
            0xDB => process_qt(&mut data, &mut img),//Quantization table, can define more than one quantization table.
            0xC0 | 0xC2 => process_start_of_frame(&mut data, &mut img, curr),// 0xC0 0xC1 0xC2 0xC3
            0xDD => process_retart_interval(&mut data, &mut img),
            invalid => panic!("invalid: {:x}", invalid),
        };

        prev = data.next_byte();
        curr = data.next_byte();
        println!("previus current: {:x}, {:x}", prev, curr);
    }

}

const zigZagMap: [usize; 64] = [
    0,   1,  8, 16,  9,  2,  3, 10,
    17, 24, 32, 25, 18, 11,  4,  5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13,  6,  7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63
];

fn process_retart_interval(it: &mut BitStream, img: &mut JpgImg) {
    let length = u16::from_be_bytes([it.next_byte(), it.next_byte()]);
    img.restart_interval = u16::from_be_bytes([it.next_byte(), it.next_byte()]);
    if length - 4 != 0 {
        println!("something wen´t wrong processing restart interval");
    }
}

fn process_start_of_frame(it: &mut BitStream, img: &mut JpgImg, value: u8) {
    //if value != 0xC0 {
    //    panic!("Currently only support baseline jpg (in future probably progressive, but not extended)");
    //}
    let length = u16::from_be_bytes([it.next_byte(), it.next_byte()]);
    let precision = it.next_byte();
    let heigth = u16::from_be_bytes([it.next_byte(), it.next_byte()]);
    let width = u16::from_be_bytes([it.next_byte(), it.next_byte()]);
    let n_components = it.next_byte();
    if n_components == 4 {
        panic!("CMYK COLOR NOT SUPPORTED");
    }
    img.height = heigth;
    img.width = width;

    let mut start_with_zero = false; //for some reason some id's in some files start with 0 instead of 1

    for _ in 0..n_components {
        let mut id = it.next_byte();
        if id == 0 {
            start_with_zero = true;
        }
        if start_with_zero {
            id += 1;
        }
        if id == 4 || id == 5 {
            panic!("YIQ NOT SUPPORTED");
        }
        let cd = &mut img.color;
        let _sampling_factor = it.next_byte();//TODO
        cd.qt_id = it.next_byte();
    }

    if length - 8 - (3 * n_components as u16) != 0 {
        panic!("read wrong number of bytes");
    }
}

fn process_appn(it: &mut BitStream) {
    println!("READING APPN MARKER");
    let length = u16::from_be_bytes([it.next_byte(), it.next_byte()]);
    println!("length: {}", length);
    for _ in 0..(length - 2) {
        it.next_byte();
    }
}

fn process_qt(it: &mut BitStream, img: &mut JpgImg) {
    println!("READING QT");
    let mut length = u16::from_be_bytes([it.next_byte(), it.next_byte()]);
    length -= 2;
    while length > 0 { // remember we can get more than one table defined here
        let table_header = it.next_byte();
        length -= 1;
        let bit_depth = 8 + (table_header >> 4) * 8;
        let table_id = (table_header & 0x0F) as usize;
        if bit_depth == 8 {
            for i in 0..64 {
                img.quantization_table[table_id][zigZagMap[i]] = it.next_byte() as u16;
            }
        }else {
            for i in 0..64 {
                img.quantization_table[table_id][zigZagMap[i]] = u16::from_be_bytes([it.next_byte(), it.next_byte()]);
            }
        }
        length = length -  64 * (bit_depth / 8) as u16;
    }
}

#[derive(Debug)]
struct CromaticData {
    rgb: bool,
    qt_id: u8,//quantization table id
}

#[derive(Debug)]
struct JpgImg {
    height: u16,
    width: u16,
    quantization_table: [[u16;64];4],
    dct_table: [HuffmanTable; 4],
    act_table: [HuffmanTable; 4],
    color: CromaticData,
    restart_interval: u16,//restart first value of coeficent table to 0 every 4 MCU
    //color component
}

impl JpgImg {
    fn new() -> JpgImg {
        Self { 
            height: 0, width: 0, 
            quantization_table: [[0; 64]; 4], 
            dct_table: [HuffmanTable::new(); 4], 
            act_table: [HuffmanTable::new(); 4],
            color: CromaticData { rgb: true, qt_id: 0 },
            restart_interval: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct HuffmanTable {
    symbol: [u8; 176],
    code: [u8; 176],
}

impl HuffmanTable {
    fn new() -> HuffmanTable {
        Self { symbol: [0; 176], code: [0; 176] }
    }
    
}


struct BitStream<'a> {
    i: usize,
    data: &'a [u8],
    bit_position: u8,
}

impl<'a> BitStream<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { i: usize::MAX, data, bit_position: 1 }
    }

    fn next_byte(&mut self) -> u8 {
        //truncate current bits if we read whole new byte.
        if self.i == usize::MAX {
            self.i = 0;
            return self.data[0];
        }
        self.bit_position = 8;
        self.i += 1;
        return self.data[self.i];
    }

    fn next_bit(&mut self) -> usize {
        if self.i == usize::MAX {
            self.i = 0;
        }
        if self.bit_position > 8 {
            self.bit_position = 1;
            self.i = self.i + 1;
        }
        let position = self.bit_position;
        self.bit_position += 1;
        return (self.data[self.i] >> (8 - position)) as usize & 1;
    }

    fn next_bits(&mut self, n: usize) -> usize {
        let mut acc = 0;
        for _ in 0..n {
            acc = (acc << 1) | self.next_bit();
        }
        return acc;
    }
}