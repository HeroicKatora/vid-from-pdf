//! Implements theora frame encoding.
//!
//! Reference document: <https://www.theora.org/doc/Theora.pdf>
//!
//! As for mapping these to the mkv codec encapsulating:
//! > Initialization: The Private Data contains the first three Theora packets in order. The
//! > lengths of the packets precedes them. The actual layout is: […]
//! > -- <https://www.matroska.org/technical/codec_specs.html>

#[repr(u8)]
pub enum Packet {
    Identification = 0x80,
    Comment = 0x81,
    Setup = 0x82,
}

pub struct Identification {
    /// Major, minor, revision
    version: (u8, u8, u8),
    /// number of macro blocks along width.
    frame_macro_width: u16,
    /// number of macro blocks along height.
    frame_macro_height: u16,
    /// 24-bit
    picture_width: u32,
    /// 24-bit
    picture_height: u32,
    /// 8-bit
    picture_x: u8,
    /// 8-bit
    picture_y: u8,
    /// 32-bit ratio.
    frame_rate: (u32, u32),
    /// 24-bit ratio.
    pixel_aspect: (u32, u32),
    /// 24-bit
    nominal_bitrate: u32,
    /// 8-bit
    colorspace: u8,
    /// 6-bit
    quality: u8,
    /// 5-bit
    /// Key frame number shift in granule position.
    /// ???
    keyframe_shift: u8,
    /// 2-bit
    pixel_format: u8,
}

pub struct Comment {
    /// The vendor comment.
    vendor: String,
    /// Maximum length: u32::MAX
    user: Vec<String>,
}

/// How color information is laid out and subsampled.
#[repr(u8)]
pub enum PixelFormat {
    Ch420 = 0,
    Ch422 = 1,
    Ch444 = 2,
}

pub struct KeyFrame {
    // TODO: Encoding parameters..

    // Color values, converted from rgb.
    y_chunks: Vec<[u8; 256]>,
    u_chunks: Vec<[u8; 256]>,
    v_chunks: Vec<[u8; 256]>,
}

#[repr(u8)]
pub enum ColorSpace {
    Unknown = 0,
    ItuRec470m = 1,
    ItuRec470bg = 2,
}

pub struct Setup {
}

impl KeyFrame {
    /// Convert RGBA pixels to Yuv encodable frame data.
    pub fn new(rgba: &[[u8; 3]], width: u32, height: u32) -> Self {
        todo!()
    }

    pub fn code_as_intra(&self, into: &mut Vec<u8>) {
        // Output of this method:
        // - FTYPE=0, NQIS, QIS
        // - coded block flags based on NSBS, NBS
        // - no macro block coding mode, skipped
        // - No motion vector computation, skipped.
        // - block-level qi values
        // - using NMBS, BCODED, HTS, DCT coefficients

        // To arrive at coefficients we do:
        // 1. loop filter each Y, Cb, Cr
        //  - output FLIMS
        // 2. split the frame into coefficient (DC, AC) blocks.
        //  2.1. compute DQS using forward DCT
        //  2.2. assign ACSCALE, DCSCALE for each
        //  2.3. perform dc, ac quantization
        //  2.4. predict dc values
        // 3. ensure NCOEFFS[bi] is 64, i.e. all components are coded

        let bitstring = todo!();
        self.rle_code(bitstring, into);
    }

    fn rle_code(&self, be_bits: &[u8], into: &mut Vec<u8>) {
        struct Position {
            byte: usize,
            bit: u8,
        }

        fn consume_equal_bits(stream: &[u8], pos: &mut Position, bit: bool) -> usize {
            loop {
            }
        }

        let mut pos = Position { byte: 0, bit: 7 };
    }
}

fn dct_filter8x8(block: &mut [i16; 64]) {
    // 1. Filter columns
    for ci in 0..8 {
        let mut x = [0; 8];
        for ri in 0..8 { x[ri] = (block[ri*8 + ci] << 4) - 8 };
        let y = dct_filter1d(&x);
        for ri in 0..8 { block[ri*8 + ci] = y[ri] };
    }

    // 2. Filter rows
    for ri in 0..8 {
        let mut x = [0; 8];
        for ci in 0..8 { x[ci] = block[ri*8 + ci] };
        let y = dct_filter1d(&x);
        for ci in 0..8 { block[ri*8 + ci] = y[ci] };
    }
}

fn dct_filter1d(x: &[i16; 8]) -> [i16; 8] {
    const C1: i32 = 64227;
    const C2: i32 = 60547;
    const C3: i32 = 54491;
    const C4: i32 = 46341;
    const C5: i32 = 36410;
    const C6: i32 = 25080;
    const C7: i32 = 12785;
    const S6: i32 = C2;
    const S7: i32 = C1;
    const S3: i32 = C4;

    // Implements the network in four stages.
    let t: [i16; 8] = [
        x[0] + x[7],
        x[1] + x[6],
        x[2] + x[5],
        x[3] + x[4],
        x[3] - x[4],
        x[2] - x[5],
        x[1] - x[6],
        x[0] - x[7],
    ];

    let t: [i16; 8] = [
        t[0] + t[3],
        t[1] + t[2],
        t[1] - t[2],
        t[0] - t[3],
        t[4],
        ((C4 * i32::from(t[5] + t[6])) / 16) as i16,
        ((C4 * i32::from(t[5] - t[6])) / 16) as i16,
        t[7],
    ];

    let t: [i16; 8] = [
        t[0] + t[1],
        t[0] - t[1],
        ((S6 * i32::from(t[3])) / 16 + (C6 * i32::from(t[2])) /16) as i16,
        ((S6 * i32::from(t[3])) / 16 - (C6 * i32::from(t[2])) /16) as i16,
        t[4] + t[5],
        t[4] - t[5],
        t[6] + t[7],
        t[6] - t[7],
    ];

    let t: [i16; 8] = [
        ((C4 * i32::from(t[0])) / 16) as i16,
        ((C4 * i32::from(t[1])) / 16) as i16,
        t[2],
        t[3],
        ((S7 * i32::from(t[7])) / 16 + (C7 * i32::from(t[4])) / 16) as i16,
        ((S3 * i32::from(t[6])) / 16 + (C3 * i32::from(t[5])) / 16) as i16,
        ((C3 * i32::from(t[6])) / 16 - (S3 * i32::from(t[5])) / 16) as i16,
        ((C7 * i32::from(t[7])) / 16 - (S7 * i32::from(t[4])) / 16) as i16,
    ];

    [
        t[0],
        t[4],
        t[2],
        t[6],
        t[1],
        t[5],
        t[3],
        t[7]
    ]
}
