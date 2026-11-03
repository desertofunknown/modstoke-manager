use crate::installation_profiles::{self, TargetKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

// Endnight asked that the Manager ship no copy of the game's art, so the Sons of the Forest skin
// reads its frame, backdrop and font from the player's own installation. The decoded files are
// cached for this user only, rebuilt when the game's files change, and served to the webview at
// gameskin://.
const DATA_FOLDER: &str = "SonsOfTheForest_Data";
const CACHE_FOLDER: &str = "game-skin";
const MANIFEST_FILE: &str = "sources.json";
// Raise when the cached files change shape, so existing caches are rebuilt.
const CACHE_FORMAT: u32 = 2;
const MAX_METADATA: usize = 64 * 1024 * 1024;
const MAX_ASSET: usize = 64 * 1024 * 1024;
const MAX_SIDE: usize = 16384;
const TEXTURE_2D: i32 = 28;
const FONT: i32 = 128;
const MONO_BEHAVIOUR: i32 = 114;
const UNREADABLE: &str = "it is not in a format this Manager can read";

struct Asset {
    cache_file: &'static str,
    class_id: i32,
    name: &'static str,
    data_file: &'static str,
    // Where the asset sits in the current game build. A later build may move it, so the name and
    // type decide, and the whole file is searched when this ID no longer matches.
    path_id: i64,
}

const FRAME_MASK: Asset = Asset {
    cache_file: "frame-mask.png",
    class_id: TEXTURE_2D,
    name: "Background (400ppu)",
    data_file: "resources.assets",
    path_id: 1277,
};
const BACKDROP: Asset = Asset {
    cache_file: "title-backdrop.png",
    class_id: TEXTURE_2D,
    name: "Blurred Title Screen",
    data_file: "sharedassets1.assets",
    path_id: 14,
};
const ROBOTO: Asset = Asset {
    cache_file: "roboto.ttf",
    class_id: FONT,
    name: "Roboto-Regular",
    data_file: "resources.assets",
    path_id: 16274,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSkin {
    // Changes whenever the cached files do, so the webview never keeps an old copy.
    revision: String,
    font: bool,
}

struct Files {
    frame_mask: Vec<u8>,
    backdrop: Vec<u8>,
    font: Option<Vec<u8>>,
}

// Names the game files a cache was built from and the format it was written in.
#[derive(Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    format: u32,
    sources: Vec<Stamp>,
    font: bool,
    // When the copy was written, so a rebuild from unchanged game files still gets a new revision.
    built: u64,
}

#[derive(Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stamp {
    path: String,
    size: u64,
    modified: u64,
}

#[tauri::command]
pub async fn read_game_skin(app: tauri::AppHandle, executable: String) -> Result<GameSkin, String> {
    let cache = app.path().app_local_data_dir()
        .map_err(|error| format!("Could not find Manager's data folder: {error}"))?
        .join(CACHE_FOLDER);
    tauri::async_runtime::spawn_blocking(move || {
        let inspected = installation_profiles::inspect_target(&executable)?;
        if inspected.target.kind != TargetKind::Player {
            return Err("The Sons of the Forest skin is read from the game, not from a dedicated server.".into());
        }
        let root = inspected.root
            .ok_or_else(|| inspected.problem.unwrap_or_else(|| "The game installation is unavailable.".into()))?;
        cached_skin(&cache, &Path::new(&root).join(DATA_FOLDER))
    })
    .await
    .map_err(|error| format!("Could not read the Sons of the Forest skin: {error}"))?
}

/// Serves the cached skin files at gameskin://, so the webview loads them like any image or font.
pub fn serve<R: tauri::Runtime>(
    context: tauri::UriSchemeContext<'_, R>,
    request: tauri::http::Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    let cache = context.app_handle().path().app_local_data_dir().map(|folder| folder.join(CACHE_FOLDER));
    let name = request.uri().path().trim_start_matches('/').to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let found = [(&FRAME_MASK, "image/png"), (&BACKDROP, "image/png"), (&ROBOTO, "font/ttf")]
            .into_iter()
            .find(|(asset, _)| asset.cache_file == name)
            .zip(cache.ok())
            .and_then(|((asset, kind), cache)| Some((fs::read(cache.join(asset.cache_file)).ok()?, kind)));
        let response = match found {
            // Masks and fonts are fetched with CORS from the app's own origin.
            Some((bytes, kind)) => tauri::http::Response::builder()
                .header("Content-Type", kind)
                .header("Access-Control-Allow-Origin", "*")
                .header("Cache-Control", "no-cache")
                .body(bytes),
            None => tauri::http::Response::builder().status(404).body(Vec::new()),
        };
        responder.respond(response.unwrap_or_default());
    });
}

fn revision(manifest: &[u8]) -> String {
    format!("{:x}", Sha256::digest(manifest))[..16].to_owned()
}

fn stamps(data: &Path) -> Vec<Stamp> {
    let mut names = Vec::new();
    for asset in [&FRAME_MASK, &BACKDROP, &ROBOTO] {
        for name in [asset.data_file.to_owned(), format!("{}.resS", asset.data_file)] {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names.iter()
        .filter_map(|name| {
            let path = data.join(name);
            let metadata = fs::metadata(&path).ok()?;
            let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
            Some(Stamp { path: path.to_string_lossy().into_owned(), size: metadata.len(), modified })
        })
        .collect()
}

fn cached_skin(cache: &Path, data: &Path) -> Result<GameSkin, String> {
    let sources = stamps(data);
    if let Ok(bytes) = fs::read(cache.join(MANIFEST_FILE)) {
        let present = |asset: &Asset| cache.join(asset.cache_file).is_file();
        if let Ok(manifest) = serde_json::from_slice::<Manifest>(&bytes) {
            if manifest.format == CACHE_FORMAT && manifest.sources == sources
                && present(&FRAME_MASK) && present(&BACKDROP) && manifest.font == present(&ROBOTO) {
                return Ok(GameSkin { revision: revision(&bytes), font: manifest.font });
            }
        }
    }
    let files = extract(data)?;
    let built = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64);
    let manifest = Manifest { format: CACHE_FORMAT, sources, font: files.font.is_some(), built };
    let bytes = store(cache, &manifest, &files)
        .map_err(|error| format!("Could not save the Sons of the Forest skin in Manager's data folder: {error}"))?;
    Ok(GameSkin { revision: revision(&bytes), font: manifest.font })
}

fn store(cache: &Path, manifest: &Manifest, files: &Files) -> std::io::Result<Vec<u8>> {
    fs::create_dir_all(cache)?;
    let _ = fs::remove_file(cache.join(MANIFEST_FILE));
    let write = |name: &str, bytes: &[u8]| -> std::io::Result<()> {
        let mut staged = tempfile::NamedTempFile::new_in(cache)?;
        staged.write_all(bytes)?;
        staged.as_file().sync_all()?;
        staged.persist(cache.join(name)).map(|_| ()).map_err(|error| error.error)
    };
    write(FRAME_MASK.cache_file, &files.frame_mask)?;
    write(BACKDROP.cache_file, &files.backdrop)?;
    match &files.font {
        Some(font) => write(ROBOTO.cache_file, font)?,
        None => { let _ = fs::remove_file(cache.join(ROBOTO.cache_file)); }
    }
    // The manifest goes last, so a half-written cache is never taken as current.
    let bytes = serde_json::to_vec(manifest).map_err(std::io::Error::other)?;
    write(MANIFEST_FILE, &bytes)?;
    Ok(bytes)
}

fn extract(data: &Path) -> Result<Files, String> {
    let mut opened: HashMap<&str, AssetsFile> = HashMap::new();
    let mut read = |asset: &Asset, decode: fn(&AssetsFile, &Object) -> Result<Vec<u8>, &'static str>| -> Result<Vec<u8>, String> {
        if !opened.contains_key(asset.data_file) {
            opened.insert(asset.data_file, AssetsFile::open(&data.join(asset.data_file))?);
        }
        let file = &opened[asset.data_file];
        let object = file.find(asset)
            .ok_or_else(|| format!("{} has no {}.", asset.data_file, asset.name))?;
        decode(file, object).map_err(|problem| format!("Could not read {} from {}: {problem}.", asset.name, asset.data_file))
    };
    Ok(Files {
        // The frame is only used as a CSS mask, which needs its alpha and no more than half its size.
        frame_mask: read(&FRAME_MASK, |file, object| file.texture(object)?.half_size_alpha_png())?,
        backdrop: read(&BACKDROP, |file, object| file.texture(object)?.png())?,
        // Without the game's Roboto the Manager falls back to Segoe UI.
        font: read(&ROBOTO, AssetsFile::font).ok(),
    })
}

struct Object {
    path_id: i64,
    offset: u64,
    size: u32,
    class_id: i32,
}

/// A Unity serialized file (format 22 and later), indexed for its textures and fonts.
struct AssetsFile {
    file: File,
    folder: PathBuf,
    little_endian: bool,
    unity: (u32, u32),
    data_offset: u64,
    objects: Vec<Object>,
}

impl AssetsFile {
    fn open(path: &Path) -> Result<Self, String> {
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        let unreadable = || format!("Could not read {name}: {UNREADABLE}.");
        let mut file = File::open(path).map_err(|error| format!("Could not open {name}: {error}"))?;
        // The header is big-endian; its endianness flag covers the metadata and objects after it.
        let mut header = [0u8; 48];
        file.read_exact(&mut header).map_err(|_| unreadable())?;
        let header_u32 = |at: usize| u32::from_be_bytes(header[at..at + 4].try_into().unwrap());
        if header_u32(8) < 22 {
            return Err(unreadable());
        }
        let little_endian = header[16] == 0;
        let metadata_size = header_u32(20) as usize;
        let data_offset = u64::from_be_bytes(header[32..40].try_into().unwrap());
        if metadata_size > MAX_METADATA {
            return Err(unreadable());
        }
        let mut metadata = vec![0; metadata_size];
        file.read_exact(&mut metadata).map_err(|_| unreadable())?;

        let mut reader = Reader { bytes: &metadata, at: 0, little_endian };
        let index = (|| -> Result<_, &'static str> {
            let version = reader.c_string()?;
            let mut parts = version.split('.').map(|part| part.parse::<u32>().unwrap_or(0));
            let unity = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
            reader.skip(4)?; // target platform
            let type_tree = reader.u8()? != 0;
            let mut classes = Vec::new();
            for _ in 0..reader.count()? {
                let class_id = reader.i32()?;
                reader.skip(3)?; // stripped flag and script type index
                reader.skip(if class_id == MONO_BEHAVIOUR { 32 } else { 16 })?; // script ID and type hash
                if type_tree {
                    let nodes = reader.count()?;
                    let strings = reader.count()?;
                    reader.skip(nodes.saturating_mul(32).saturating_add(strings))?;
                    let dependencies = reader.count()?;
                    reader.skip(dependencies.saturating_mul(4))?;
                }
                classes.push(class_id);
            }
            let mut objects = Vec::new();
            for _ in 0..reader.count()? {
                reader.align();
                let path_id = reader.i64()?;
                let offset = reader.u64()?;
                let size = reader.u32()?;
                let class_id = *classes.get(reader.count()?).ok_or(UNREADABLE)?;
                if class_id == TEXTURE_2D || class_id == FONT {
                    objects.push(Object { path_id, offset, size, class_id });
                }
            }
            Ok((unity, objects))
        })();
        let (unity, objects) = index.map_err(|_| unreadable())?;
        Ok(Self { file, folder: path.parent().unwrap_or(Path::new("")).to_path_buf(), little_endian, unity, data_offset, objects })
    }

    fn object_bytes(&self, object: &Object, limit: usize) -> Result<Vec<u8>, &'static str> {
        let mut file = &self.file;
        let start = self.data_offset.checked_add(object.offset).ok_or(UNREADABLE)?;
        file.seek(SeekFrom::Start(start)).map_err(|_| UNREADABLE)?;
        let mut bytes = vec![0; (object.size as usize).min(limit)];
        file.read_exact(&mut bytes).map_err(|_| UNREADABLE)?;
        Ok(bytes)
    }

    fn name(&self, object: &Object) -> Option<String> {
        // Every named object starts with its name, which is short.
        let bytes = self.object_bytes(object, 260).ok()?;
        Reader { bytes: &bytes, at: 0, little_endian: self.little_endian }.aligned_string().ok()
    }

    fn find(&self, asset: &Asset) -> Option<&Object> {
        let matches = |object: &&Object| object.class_id == asset.class_id && self.name(object).as_deref() == Some(asset.name);
        self.objects.iter().filter(|object| object.path_id == asset.path_id).find(matches)
            .or_else(|| self.objects.iter().find(matches))
    }

    fn texture(&self, object: &Object) -> Result<Image, &'static str> {
        // Texture2D's fields as Unity 2022.2 and later write them, without a type tree.
        if self.unity < (2022, 2) {
            return Err(UNREADABLE);
        }
        let bytes = self.object_bytes(object, MAX_ASSET)?;
        let mut reader = Reader { bytes: &bytes, at: 0, little_endian: self.little_endian };
        reader.aligned_string()?; // name
        reader.skip(6)?; // forced fallback format, downscale fallback, optional alpha channel
        reader.align();
        let width = reader.count()?;
        let height = reader.count()?;
        reader.skip(8)?; // complete image size, stripped mips
        let format = reader.i32()?;
        reader.skip(7)?; // mip count, readable, preprocessed, ignore mipmap limit
        reader.align();
        reader.aligned_string()?; // mipmap limit group
        reader.skip(1)?; // streaming mipmaps
        reader.align();
        reader.skip(4)?; // streaming priority
        let image_count = reader.i32()?;
        let dimension = reader.i32()?;
        reader.skip(32)?; // texture settings, lightmap format, colour space
        let platform_blob = reader.count()?;
        reader.skip(platform_blob)?;
        reader.align();
        let inline = reader.count()?;
        if image_count != 1 || dimension != 2 || !(1..=MAX_SIDE).contains(&width) || !(1..=MAX_SIDE).contains(&height) {
            return Err(UNREADABLE);
        }
        let data = if inline > 0 {
            reader.take(inline)?.to_vec()
        } else {
            let offset = reader.u64()?;
            let size = reader.u32()? as usize;
            let path = reader.aligned_string()?;
            self.stream(&path, offset, size)?
        };
        // DXT1, DXT5 and BC7. The data must cover the size before the pixels are allocated.
        let block_bytes = match format {
            10 => 8,
            12 | 25 => 16,
            _ => return Err("its texture format is one this Manager does not decode"),
        };
        if data.len() < width.div_ceil(4) * height.div_ceil(4) * block_bytes {
            return Err(UNREADABLE);
        }
        let mut pixels = vec![0u32; width * height];
        match format {
            10 => texture2ddecoder::decode_bc1(&data, width, height, &mut pixels),
            12 => texture2ddecoder::decode_bc3(&data, width, height, &mut pixels),
            _ => texture2ddecoder::decode_bc7(&data, width, height, &mut pixels),
        }?;
        // Unity stores rows bottom up, and the decoder writes BGRA.
        let mut rgba = vec![0u8; width * height * 4];
        for (target, source) in rgba.chunks_exact_mut(width * 4).zip(pixels.chunks_exact(width).rev()) {
            for (out, pixel) in target.as_chunks_mut::<4>().0.iter_mut().zip(source) {
                let [blue, green, red, alpha] = pixel.to_le_bytes();
                out.copy_from_slice(&[red, green, blue, alpha]);
            }
        }
        Ok(Image { width, height, rgba })
    }

    fn stream(&self, path: &str, offset: u64, size: usize) -> Result<Vec<u8>, &'static str> {
        // Texture data sits in a .resS file next to the asset file, and nothing else is read.
        if path.contains(['/', '\\', ':']) || !path.ends_with(".resS") || size > MAX_ASSET {
            return Err(UNREADABLE);
        }
        let mut file = File::open(self.folder.join(path)).map_err(|_| "its texture data file could not be opened")?;
        file.seek(SeekFrom::Start(offset)).map_err(|_| UNREADABLE)?;
        let mut bytes = vec![0; size];
        file.read_exact(&mut bytes).map_err(|_| UNREADABLE)?;
        Ok(bytes)
    }

    fn font(&self, object: &Object) -> Result<Vec<u8>, &'static str> {
        let bytes = self.object_bytes(object, MAX_ASSET)?;
        let mut reader = Reader { bytes: &bytes, at: 0, little_endian: self.little_endian };
        reader.aligned_string()?; // name
        reader.skip(32)?; // line spacing, default material, font size, texture
        reader.skip(20)?; // ASCII start offset, tracking, character spacing and padding, case conversion
        let characters = reader.count()?;
        reader.skip(characters.saturating_mul(44))?;
        let kerning = reader.count()?;
        reader.skip(kerning.saturating_mul(8))?;
        reader.skip(4)?; // pixel scale
        let length = reader.count()?;
        let font = reader.take(length)?;
        if !matches!(font.get(..4), Some([0, 1, 0, 0] | b"OTTO" | b"true")) {
            return Err("it holds no TrueType font");
        }
        Ok(font.to_vec())
    }
}

/// A decoded texture, top row first.
struct Image {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl Image {
    fn png(&self) -> Result<Vec<u8>, &'static str> {
        encode_png(self.width, self.height, png::ColorType::Rgba, &self.rgba)
    }

    fn half_size_alpha_png(&self) -> Result<Vec<u8>, &'static str> {
        let (width, height) = ((self.width / 2).max(1), (self.height / 2).max(1));
        let alpha = |x: usize, y: usize| u32::from(self.rgba[(y.min(self.height - 1) * self.width + x.min(self.width - 1)) * 4 + 3]);
        let mut pixels = Vec::with_capacity(width * height * 2);
        for y in 0..height {
            for x in 0..width {
                let sum = alpha(2 * x, 2 * y) + alpha(2 * x + 1, 2 * y) + alpha(2 * x, 2 * y + 1) + alpha(2 * x + 1, 2 * y + 1);
                pixels.extend_from_slice(&[255, ((sum + 2) / 4) as u8]);
            }
        }
        encode_png(width, height, png::ColorType::GrayscaleAlpha, &pixels)
    }
}

fn encode_png(width: usize, height: usize, color: png::ColorType, data: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut encoded = Vec::new();
    let mut encoder = png::Encoder::new(&mut encoded, width as u32, height as u32);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    // The cache is local, so encoding speed matters more than size.
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(|_| "the image could not be encoded")?;
    writer.write_image_data(data).map_err(|_| "the image could not be encoded")?;
    writer.finish().map_err(|_| "the image could not be encoded")?;
    Ok(encoded)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    little_endian: bool,
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], &'static str> {
        let end = self.at.checked_add(length).filter(|end| *end <= self.bytes.len()).ok_or(UNREADABLE)?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn skip(&mut self, length: usize) -> Result<(), &'static str> {
        self.take(length).map(|_| ())
    }

    fn align(&mut self) {
        self.at = self.at.next_multiple_of(4);
    }

    fn u8(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, &'static str> {
        let bytes = self.take(4)?.try_into().unwrap();
        Ok(if self.little_endian { u32::from_le_bytes(bytes) } else { u32::from_be_bytes(bytes) })
    }

    fn u64(&mut self) -> Result<u64, &'static str> {
        let bytes = self.take(8)?.try_into().unwrap();
        Ok(if self.little_endian { u64::from_le_bytes(bytes) } else { u64::from_be_bytes(bytes) })
    }

    fn i32(&mut self) -> Result<i32, &'static str> {
        self.u32().map(|value| value as i32)
    }

    fn i64(&mut self) -> Result<i64, &'static str> {
        self.u64().map(|value| value as i64)
    }

    fn count(&mut self) -> Result<usize, &'static str> {
        usize::try_from(self.i32()?).map_err(|_| UNREADABLE)
    }

    fn c_string(&mut self) -> Result<String, &'static str> {
        let rest = &self.bytes[self.at.min(self.bytes.len())..];
        let end = rest.iter().position(|byte| *byte == 0).ok_or(UNREADABLE)?;
        self.at += end + 1;
        Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
    }

    fn aligned_string(&mut self) -> Result<String, &'static str> {
        let length = self.count()?;
        let text = String::from_utf8_lossy(self.take(length)?).into_owned();
        self.align();
        Ok(text)
    }
}
