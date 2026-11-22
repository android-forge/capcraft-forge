// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! The fonts not in the app: display faces few titles use, downloaded
//! together when they are wanted so the app stays small. They come from
//! Google Fonts' own repository at one commit, each checked against its
//! digest, and are the very files the desktop engine bundles (SIL Open Font
//! License, Kosugi Maru under Apache 2.0).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::speech::SpeechListener;
use crate::{MobileError, context};

struct More {
    file: &'static str,
    path: &'static str,
    bytes: u64,
    sha256: &'static str,
}

/// Where they are, at the commit their digests are of.
const FROM: &str = "https://raw.githubusercontent.com/google/fonts/06bfe0d62a98375cb09c942fa18e68bd32cff057/";

const MORE: &[More] = &[
    More {
        file: "KosugiMaru-Regular.ttf",
        path: "apache/kosugimaru/KosugiMaru-Regular.ttf",
        bytes: 3_565_716,
        sha256: "4b8d0022c8dadd090ef67cd1f71f130714767af7806cba2eb4ebe4b0271c1d68",
    },
    More {
        file: "RubikScribble-Regular.ttf",
        path: "ofl/rubikscribble/RubikScribble-Regular.ttf",
        bytes: 583_828,
        sha256: "341d083481febe704791212f560a0898039d6a5744c8819102f3ce7a7f8fc2d4",
    },
    More {
        file: "Lobster-Regular.ttf",
        path: "ofl/lobster/Lobster-Regular.ttf",
        bytes: 406_076,
        sha256: "d6568e697fd50cedc0be04d8aae4127fe95add607e7bff954ca88604be80c205",
    },
    More {
        file: "CaveatBrush-Regular.ttf",
        path: "ofl/caveatbrush/CaveatBrush-Regular.ttf",
        bytes: 295_568,
        sha256: "e565bd7bffd895ca64f8d1cb77c816b9b38d9c010246386a07c9648592f6f282",
    },
    More {
        file: "EmilysCandy-Regular.ttf",
        path: "ofl/emilyscandy/EmilysCandy-Regular.ttf",
        bytes: 237_216,
        sha256: "0cf3ca9dbe71b33596e8a1eb12b40deac2e6414fe5616dda50c554580b6e0c68",
    },
    More {
        file: "Frijole-Regular.ttf",
        path: "ofl/frijole/Frijole-Regular.ttf",
        bytes: 227_860,
        sha256: "e182bbad0e1ba7eb06d93edb1c97a71f5b7d56e27433fb375f5f7138399ab72c",
    },
    More {
        file: "BlackOpsOne-Regular.ttf",
        path: "ofl/blackopsone/BlackOpsOne-Regular.ttf",
        bytes: 166_532,
        sha256: "282a825b5f294377387e3969f765408157dbea8da0f5d0aae68c6bc704b145b3",
    },
    More {
        file: "GoogleSansCode-Variable.ttf",
        path: "ofl/googlesanscode/GoogleSansCode%5Bwght%5D.ttf",
        bytes: 126_224,
        sha256: "c9649573afcd966f61096b1e9c3c3115e7b54315ca2adf9839c6f8b17d5f8e1f",
    },
    More {
        file: "BungeeHairline-Regular.ttf",
        path: "ofl/bungeehairline/BungeeHairline-Regular.ttf",
        bytes: 104_700,
        sha256: "fa720052780a73c9e8ba196bbb5332d361d366624f13f486beff125097fdbfb2",
    },
    More {
        file: "HammersmithOne-Regular.ttf",
        path: "ofl/hammersmithone/HammersmithOne-Regular.ttf",
        bytes: 102_884,
        sha256: "6faf4e9a59a2b25e1016c4e47cf4da4a494d07aee15dc2f708a88caae015ff8a",
    },
    More {
        file: "LondrinaSolid-Regular.ttf",
        path: "ofl/londrinasolid/LondrinaSolid-Regular.ttf",
        bytes: 92_132,
        sha256: "e82b9b3ee21ccf153dcf67e5f4226a210474591a6ad31b5f17f2f14dc2524514",
    },
    More {
        file: "LondrinaSolid-Black.ttf",
        path: "ofl/londrinasolid/LondrinaSolid-Black.ttf",
        bytes: 93_408,
        sha256: "8c63a15e3fa34e0b362abeb903944a2bd45e71b1efa122486c0a47e295eae06f",
    },
    More {
        file: "ZenTokyoZoo-Regular.ttf",
        path: "ofl/zentokyozoo/ZenTokyoZoo-Regular.ttf",
        bytes: 92_176,
        sha256: "9dd9dc1a5eba800aca9b0bf4671bda880b7caa6d19174c82776256f6732d604a",
    },
    More {
        file: "SecularOne-Regular.ttf",
        path: "ofl/secularone/SecularOne-Regular.ttf",
        bytes: 83_916,
        sha256: "2f45092a521db5042887941edfef73178ed7c5ba7fdfc5b6f581d44aab7b0234",
    },
    More {
        file: "PixelifySans-Variable.ttf",
        path: "ofl/pixelifysans/PixelifySans%5Bwght%5D.ttf",
        bytes: 79_160,
        sha256: "9ba86cd010a4de309d263ceff8e8044092c9db7efda869620cb9ff1c4389e8a5",
    },
    More {
        file: "AtomicAge-Regular.ttf",
        path: "ofl/atomicage/AtomicAge-Regular.ttf",
        bytes: 62_592,
        sha256: "5ea6111180af882b0799a1e1c04b7acc557807ff76e0cc24258b259a06f67ed2",
    },
    More {
        file: "MysteryQuest-Regular.ttf",
        path: "ofl/mysteryquest/MysteryQuest-Regular.ttf",
        bytes: 42_124,
        sha256: "fc3fba61be98d7de03964b41175ee5629bcc6958b251f8037ef13eabb5431eb2",
    },
    More {
        file: "ShadowsIntoLightTwo-Regular.ttf",
        path: "ofl/shadowsintolighttwo/ShadowsIntoLightTwo-Regular.ttf",
        bytes: 39_396,
        sha256: "d2540170b1b1df3094de433b4632755ba91ecb65626202a3c8affd2245e08405",
    },
    More {
        file: "GermaniaOne-Regular.ttf",
        path: "ofl/germaniaone/GermaniaOne-Regular.ttf",
        bytes: 33_240,
        sha256: "3ee3d56584bab8c9e3f59cc3360aa8a36e219dcf0a9f3c8928e3da5a19277e3f",
    },
    More {
        file: "Staatliches-Regular.ttf",
        path: "ofl/staatliches/Staatliches-Regular.ttf",
        bytes: 63_316,
        sha256: "8395212aa4c6c3534bd39a745d956305ff080c3f3ed73ba61e4fbaae951e55cc",
    },
];

/// The folder they are kept in, under the engine's own.
pub(crate) fn folder(data: &Path) -> PathBuf {
    data.join("fonts")
}

fn here(folder: &Path, more: &More) -> bool {
    std::fs::metadata(folder.join(more.file)).is_ok_and(|meta| meta.len() == more.bytes)
}

/// How many bytes of fonts there are still to download; 0 when all are here.
#[uniffi::export]
pub fn more_fonts_download() -> u64 {
    let Ok(context) = context() else { return 0 };
    let folder = folder(&context.dirs.data);
    MORE.iter()
        .filter(|more| !here(&folder, more))
        .map(|more| more.bytes)
        .sum()
}

/// Downloads the fonts not here yet, on a thread of its own; every title is
/// painted again with them.
#[uniffi::export]
pub fn download_more_fonts(listener: Arc<dyn SpeechListener>) -> Result<(), MobileError> {
    let folder = folder(&context()?.dirs.data);
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("could not create {}: {error}", folder.display()))?;
    static BUSY: AtomicBool = AtomicBool::new(false);
    if BUSY.swap(true, Ordering::Relaxed) {
        return Err(MobileError::Busy(
            "The fonts are being downloaded already.".to_owned(),
        ));
    }
    std::thread::Builder::new()
        .name("capcraft-fonts".to_owned())
        .spawn(move || {
            let wanted: Vec<&More> = MORE.iter().filter(|more| !here(&folder, more)).collect();
            let all: u64 = wanted.iter().map(|more| more.bytes).sum::<u64>().max(1);
            let mut done = 0u64;
            let never = AtomicBool::new(false);
            let mut result = Ok(());
            for more in wanted {
                let partial = folder.join(format!("{}.part", more.file));
                let fetched = concat_host::models::fetch_one(
                    &format!("{FROM}{}", more.path),
                    &partial,
                    more.bytes,
                    more.sha256,
                    &never,
                    "Stopped.",
                    &mut |received, _| {
                        listener.on_progress(
                            ((done + received) as f32 / all as f32).min(1.0),
                            "download".to_owned(),
                        );
                    },
                )
                .and_then(|()| {
                    std::fs::rename(&partial, folder.join(more.file))
                        .map_err(|error| format!("could not keep the font: {error}"))
                });
                if let Err(message) = fetched {
                    result = Err(message);
                    break;
                }
                done += more.bytes;
            }
            BUSY.store(false, Ordering::Relaxed);
            crate::frames::forget_faces();
            if let Ok(context) = context() {
                context.titles.renew();
            }
            match result {
                Ok(()) => listener.on_done(None),
                Err(message) => listener.on_failed(message),
            }
        })
        .map_err(|error| {
            BUSY.store(false, Ordering::Relaxed);
            MobileError::Failed(format!("could not start: {error}"))
        })?;
    Ok(())
}
