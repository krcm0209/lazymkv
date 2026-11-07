//! The bitmap subtitle pipeline of design part 10.1, as pure functions of bytes (R-A5): the Matroska reader of
//! evidence.mks (`mkv`), the decoders, the preparation and the row line finder. Nothing here reads a file, a process
//! or a clock (RP19).

pub mod mkv;
pub mod pgs;
pub mod vobsub;

/// The two bitmap subtitle codecs the worker reads (design part 10.3); any other codec is refused by the caller
/// (T5-R11). Def: T5-3. Use: T5-4, T5-8, T5-11.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubCodec {
    Pgs,
    VobSub,
}

/// Matroska's codec ID of PGS (Presentation Graphic Stream) subtitles (design part 10.3). Def: T5-3. Use: T5-8,
/// T5-15.
pub const CODEC_PGS: &str = "S_HDMV/PGS";

/// Matroska's codec ID of VobSub (the DVD, Digital Versatile Disc, bitmap format) subtitles (design part 10.3). Def:
/// T5-3. Use: T5-8, T5-15.
pub const CODEC_VOBSUB: &str = "S_VOBSUB";

/// One positioned object of a decoded subtitle event (design part 10.4: "a list of positioned objects, each with
/// indexed pixels, the palette and alpha"). `x` and `y` place its top-left pixel on the video frame; `indices` holds
/// `width * height` palette indices, row by row from the top; every index is below `palette.len()`. Def: T5-4.
/// Use: T5-5, T5-8, T5-11.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    /// The object's left column on the video frame. Def: T5-4. Use: T5-5.
    pub x: u32,
    /// The object's top row on the video frame. Def: T5-4. Use: T5-5.
    pub y: u32,
    /// The object's width in pixels, after the composition's cropping. Def: T5-4. Use: T5-5.
    pub width: u32,
    /// The object's height in pixels, after the composition's cropping. Def: T5-4. Use: T5-5.
    pub height: u32,
    /// The palette index of each pixel, row by row. Def: T5-4. Use: T5-5.
    pub indices: Vec<u8>,
    /// `palette[i]` is `[Y, alpha]` of index `i`, both 0 to 255: 256 entries for PGS (Presentation Graphic Stream),
    /// an undefined entry `[0, 0]`; 4 entries for VobSub, one per pixel code. Def: T5-4. Use: T5-5.
    pub palette: Vec<[u8; 2]>,
}

/// One decoded subtitle event: the video frame's size (the PGS composition's, the VobSub idx header's) and the
/// event's objects in the composition's order (design part 10.4; R2-decoders-39: trigger 4 needs the frame).
/// Def: T5-4. Use: T5-5, T5-8, T5-11.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    /// The video frame's width. Def: T5-4. Use: T5-5.
    pub video_width: u32,
    /// The video frame's height. Def: T5-4. Use: T5-5.
    pub video_height: u32,
    /// The event's objects, none for a composition that places none. Def: T5-4. Use: T5-5.
    pub objects: Vec<Object>,
}
