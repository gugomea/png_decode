// inflate() method -> unwrap zlib header -> once I have compressed data
//                  -> huffman -> lz77 -> be happy.

//RFC 1950: https://www.rfc-editor.org/rfc/rfc1950.
//RFC 1951: https://www.rfc-editor.org/rfc/rfc1951.
//Concepts: https://www.youtube.com/watch?v=oi2lMBBjQ8s&t=6538s.
//Good post: https://pyokagan.name/blog/2019-10-18-zlibinflate/. I used this blog to 
// make my implementation cleaner, before this I didn´t have BitStream, and it all looked like a mess.
#![allow(non_snake_case)]


pub fn inflate(data: &[u8]) -> Vec<u8> {
    let _CMF = data[0];// don´t care
    let _FLG = data[1];// + L

    let compressed_data = data[2..data.len() - 4].to_vec();

    let _ADLER32 = u32::from_le_bytes(data[data.len()-4..].try_into().expect("msg"));// + RATIO

    return decompress(compressed_data);
}

struct BitStream<'a> {
    i: usize,
    data: &'a [u8],
    bit_position: u8,
}
impl<'a> BitStream<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { i: 0, data, bit_position: 8 }
    }
    fn next_byte(&mut self) -> u8{
        //truncate current bits if we read whole new byte.
        self.bit_position = 8;
        self.i += 1;
        return self.data[self.i];
    }
    fn next_bit(&mut self) -> usize {
        if self.bit_position < 1 {
            self.bit_position = 8;
            self.i = self.i + 1;
        }
        let position = self.bit_position;
        self.bit_position -= 1;
        return (self.data[self.i] >> (8 - position)) as usize & 1;
    }
    fn next_bits(&mut self, n: usize) -> usize {
        let mut acc = 0;
        for i in 0..n {
            acc = acc | (self.next_bit() << i);
        }
        return acc;
    }
}

// we are with little indian now, but huffman codes!

//A compressed data set consists of a series of blocks, corresponding
//to successive blocks of input data.  The block sizes are arbitrary,
//except that non-compressible blocks are limited to 65,535 bytes.


const LENGTH: [usize; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA: [usize; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const BACKWARDS: [usize; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const BACKWARDS_EXTRA: [usize; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const CL_TABLE: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];


fn decompress(stream: Vec<u8>) -> Vec<u8> {
    let mut last_block = false;
    let mut res = Vec::with_capacity(1000000000);
    let mut it = BitStream::new(&stream);
    let mut borrar = 0;

    while !last_block {
        let BFINAL = it.next_bit();
        last_block = BFINAL == 1;
        let BTYPE = it.next_bits(2);
        if BTYPE == 0 {
            let LEN = u16::from_le_bytes([it.next_byte(), it.next_byte()]);
            let _NLEN = u16::from_le_bytes([it.next_byte(), it.next_byte()]);
            for _ in 0..LEN {
                res.push(it.next_byte());
            }
        }else {
            let (mut ll_tree, mut d_tree) = fixed_trees();
            if BTYPE == 2 {
                borrar += 1;
                let HLIT = it.next_bits(5);
                let HDIST = it.next_bits(5);
                let HCLEN = it.next_bits(4);
                let mut cl_list = vec![0; 19];// 0 so we can truncate

                for i in 0..HCLEN+4 {
                    cl_list[CL_TABLE[i]] = it.next_bits(3);
                }

                let cl_tree = huffman_from_bit_length(&cl_list, 18);

                let mut all = vec![];
                while all.len() < (HLIT + HDIST + 258) {
                    let symbol = process_symbol(&mut it, &cl_tree);
                    if symbol <= 15 {
                        all.push(symbol as usize);
                    }else if symbol == 16 {
                        let prev = *all.last().unwrap() as u8;
                        let times = it.next_bits(2) + 3;
                        all.append(&mut vec![prev as usize;times]);
                    }else if symbol == 17 {
                        let times = it.next_bits(3) + 3;
                        all.append(&mut vec![0;times]);
                    }else if symbol == 18 {
                        let times = it.next_bits(7) + 11;
                        all.append(&mut vec![0;times]);
                    }
                }
                (ll_tree, d_tree) = (huffman_from_bit_length(&all[..(HLIT + 257)], 285),
                                      huffman_from_bit_length(&all[(HLIT + 257)..], 285));
            }
            loop {
                let symbol = process_symbol(&mut it, &ll_tree);
                match symbol {
                    0..=255 => res.push(symbol as u8),
                    256 => break,
                    _ => {
                        let length = LENGTH[symbol as usize -257] + it.next_bits(LENGTH_EXTRA[symbol as usize -257]);
                        let d = process_symbol(&mut it, &d_tree);
                        let distance = BACKWARDS[d as usize] + it.next_bits(BACKWARDS_EXTRA[d as usize]);
                        for _ in 0..length {
                            res.push(res[res.len()-distance]);
                        }
                    },
                }
            }
        }
    }
    println!("huffman dinamicos creados: {}", borrar);
    return res;
}

fn process_symbol(it: &mut BitStream, tree: &HuffmanTree) -> u16 {
    // if we find a 0 we go to the left, to the rigth if is a 1.
    let mut current = tree;
    while current.left != None && current.rigth != None {
        let nb = it.next_bit();
        match nb == 1 {
            true => current = current.rigth.as_ref().unwrap(),
            false => current = current.left.as_ref().unwrap(),
        };
    }
    return current.symbol;
}

#[derive(PartialEq, Clone, Debug)]
struct HuffmanTree {
    symbol: u16,
    left: Option<Box<HuffmanTree>>,
    rigth: Option<Box<HuffmanTree>>,
}

impl HuffmanTree {
    fn new() -> Self {
        Self { symbol: 0, left: None, rigth: None }
    }

    fn add_code(&mut self, code: u16, length: usize, symbol: u16) {
        // if we find a 0 we go to the left, to the rigth if is a 1.
        let mut current = self;
        for i in (0..length).rev() {
            let bit = code & (1 << i);
            if bit != 0 {
                if let None = &mut current.rigth {// it is posible that we have not processed this code yet
                    current.rigth = Some(Box::new(Self::new()));
                }
                current = current.rigth.as_mut().unwrap();
            }else{
                if let None = &mut current.left {
                    current.left = Some(Box::new(Self::new()));
                }
                current = current.left.as_mut().unwrap();
            }
        }
        current.symbol = symbol;
    }
}

fn huffman_from_bit_length(bit_lengths: &[usize], until: u16) -> HuffmanTree {
    //we can define the Huffman tree for an alphabet
    //just by giving the bit lengths of the codes for each symbol of
    //the alphabet in order; this is sufficient to determine the
    //actual codes. Because this huffman codes follow these constraints:
    //         * All codes of a given bit length have lexicographically
    //           consecutive values, in the same order as the symbols
    //           they represent;

    //         * Shorter codes lexicographically precede longer codes.
    let symbols = (0..=until).collect::<Vec<u16>>();
    let mut huffman = HuffmanTree::new();
    let MAX_BITS = *bit_lengths.iter().max().unwrap();
    let mut bl_count = vec![0; MAX_BITS + 1];
    let mut next_code = vec![0; MAX_BITS + 1];
    bit_lengths.iter().for_each(|&x| bl_count[x] += 1);
    let mut code = 0;
    bl_count[0] = 0;
    for bits in 1..=MAX_BITS {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }

    for n in 0..usize::min(symbols.len(), bit_lengths.len()) {
        let len = bit_lengths[n];
        if len != 0 {
            huffman.add_code(next_code[len], len, symbols[n]);// idk if this is right
            next_code[len] += 1;
        }
    }
    return huffman;
}

fn fixed_trees() -> (HuffmanTree, HuffmanTree) {
    //The Huffman codes for the two alphabets are fixed, and are not
    //represented explicitly in the data.  The Huffman code lengths
    //for the literal/length alphabet are:
    let mut literal_length = vec![8usize; 288];
    let back_distance = vec![5usize; 32];

    for x in 144..256 {
        literal_length[x] = 9;
    }

    for x in 256..280 {
        literal_length[x] = 7;
    }
    //Literal/length values 286-287 will never actually
    //occur in the compressed data, but participate in the code
    //construction.

    //Note that distance codes 30-
    //31 will never actually occur in the compressed data.
    return (huffman_from_bit_length(&literal_length, 285u16),
            huffman_from_bit_length(&back_distance, 29u16));
}