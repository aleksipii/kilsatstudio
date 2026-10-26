# A01 — Riippuvuudet 26.10.2026

Lähtöcommit: `773ac9417ee000668fd4f92750e25797a9f57e34`. Manifestit ja lukitustiedosto tarkistettu; ei asennetun macOS-jakelun inventaario.

## Suorat ulkoiset Rust-riippuvuudet

| Riippuvuus | Lukitut versiot | Perustelu | Käyttäjät | Tarkistustila |
| --- | --- | --- | --- | --- |
| base64 | 0.22.1 | Kuva-aineiston tekstikoodaus | motion-art (dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| eframe | 0.31.1 | egui-työpöytäsovelluksen kehys | studio-desktop (dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| flate2 | 1.1.10 | Pakatun kieliaineiston käsittely | motion-lipsync (dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| hound | 3.5.1 | PCM WAV -luku ja kirjoitus | motion-audio (dependencies), motion-tts (dependencies), motion-voice (dependencies), studio-desktop (dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| image | 0.25.10 | PNG/JPEG-kuva-aineisto | motion-art (dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| resvg | 0.45.1 | SVG:n rasterointi vientiin | motion-export (dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| rodio | 0.20.1 | macOS-äänentoisto | studio-desktop (cfg(target_os = "macos")) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| rustybuzz | 0.20.1 | GPU-tekstin muotoilu | motion-render (dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| serde | 1.0.229 | Tietorakenteiden serialisointi | motion-audio (dependencies), motion-tts (dependencies), motion-lipsync (dependencies), motion-voice (dependencies), motion-script (dependencies), studio-desktop (dependencies), motion-scene (dependencies), motion-export (dependencies), motion-project (dependencies), motion-rig (dependencies), motion-animation (dependencies), motion-art (dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| serde_json | 1.0.154 | JSON-projektit, protokollat ja metatiedot | motion-tts (dependencies), motion-lipsync (dependencies), motion-voice (dependencies), studio-desktop (dependencies), motion-scene (dev-dependencies), motion-render (dev-dependencies), motion-export (dependencies), motion-project (dependencies), motion-art (dev-dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| serde_yaml | 0.9.34+deprecated | MotionScript YAML -jäsennys | motion-script (dependencies), studio-desktop (dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| sha2 | 0.10.9 | Sisältötunnisteet ja eheystarkistus | motion-tts (dependencies), motion-lipsync (dependencies), studio-desktop (dependencies), motion-render (dev-dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| thiserror | 1.0.69, 2.0.21 | Tyypitetyt virheet | motion-script (dependencies), motion-scene (dependencies), motion-render (dependencies), motion-export (dependencies), motion-project (dependencies), motion-rig (dependencies), motion-animation (dependencies), motion-art (dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| ttf-parser | 0.25.1 | Fonttien jäsennys | motion-render (dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |
| wgpu | 24.0.5 | Valinnainen GPU/Metal-renderöintipolku | motion-render (dependencies) | Manifesti/lock tarkistettu; lisenssitekstit ja jakeluvelvoitteet avoinna |

## Workspace-moduulien riippuvuusrajat

Normaalit ja kohdekohtaiset riippuvuudet; testiriippuvuudet eivät kuulu tähän DAG-tarkistukseen. Kaikki 14 moduulia ilmoittavat projektin MIT-lisenssin.

| Moduuli | Riippuu workspace-moduuleista |
| --- | --- |
| motion-animation | motion-rig |
| motion-art | motion-jobs |
| motion-audio | Ei workspace-riippuvuuksia |
| motion-export | motion-art, motion-audio, motion-jobs, motion-lipsync, motion-project, motion-render, motion-scene, motion-script, motion-tts, motion-voice |
| motion-jobs | Ei workspace-riippuvuuksia |
| motion-lipsync | motion-audio |
| motion-project | motion-art, motion-script |
| motion-render | motion-art, motion-lipsync, motion-rig, motion-scene, motion-script |
| motion-rig | Ei workspace-riippuvuuksia |
| motion-scene | motion-animation, motion-lipsync, motion-rig, motion-script |
| motion-script | motion-animation, motion-lipsync, motion-rig |
| motion-tts | Ei workspace-riippuvuuksia |
| motion-voice | motion-audio, motion-jobs, motion-lipsync, motion-scene, motion-script, motion-tts |
| studio-desktop | motion-animation, motion-art, motion-audio, motion-export, motion-jobs, motion-lipsync, motion-project, motion-rig, motion-scene, motion-script, motion-tts, motion-voice |

## Kaikki Cargo.lock-paketit

Jokaisen välillisen paketin perustelu on sen lukitustiedostosta löytyvä riippuvuussuhde. Se ei vielä osoita paketin tarpeellisuutta lopullisessa macOS-jakelussa. Kohde- ja ominaisuuskohtainen rajaus on avoin. JSON-inventaario säilyttää lähteet ja tarkistussummat.

| Paketti | Versio | Perustelu / riippuvuuden tuovat paketit | Tarkistustila |
| --- | --- | --- | --- |
| ab_glyph | 0.2.32 | epaint 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| ab_glyph_rasterizer | 0.1.10 | ab_glyph 0.2.32 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| adler2 | 2.0.1 | miniz_oxide 0.8.9; miniz_oxide 0.9.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| ahash | 0.8.12 | eframe 0.31.1; egui 0.31.1; egui-wgpu 0.31.1; egui-winit 0.31.1; egui_glow 0.31.1; epaint 0.31.1; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| aho-corasick | 1.1.5 | regex 1.13.1; regex-automata 0.4.18 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| alsa | 0.9.1 | cpal 0.15.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| alsa-sys | 0.3.1 | alsa 0.9.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| android-activity | 0.6.1 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| android-properties | 0.2.2 | android-activity 0.6.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| android_system_properties | 0.1.6 | wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| arboard | 3.6.1 | egui-winit 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| arrayref | 0.3.9 | tiny-skia 0.11.4; tiny-skia-path 0.11.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| arrayvec | 0.7.8 | kurbo 0.11.3; naga 24.0.0; tiny-skia 0.11.4; wgpu 24.0.5; wgpu-core 24.0.5; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| as-raw-xcb-connection | 1.0.1 | x11rb 0.13.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| ash | 0.38.0+1.3.281 | wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| atomic-waker | 1.1.2 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| autocfg | 1.5.1 | memoffset 0.9.1; num-traits 0.2.19 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| base64 | 0.22.1 | motion-art 0.1.0; usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| bindgen | 0.72.1 | coreaudio-sys 0.2.18 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| bit-set | 0.8.0 | naga 24.0.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| bit-vec | 0.8.0 | bit-set 0.8.0; wgpu-core 24.0.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| bitflags | 1.3.2 | core-graphics 0.23.2; core-graphics-types 0.1.3; coreaudio-rs 0.11.3; png 0.17.16; redox_syscall 0.4.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| bitflags | 2.13.2 | alsa 0.9.1; android-activity 0.6.1; bindgen 0.72.1; calloop 0.13.0; calloop 0.14.5; dispatch2 0.3.1; egui 0.31.1; glutin 0.32.3; gpu-alloc 0.6.2; gpu-alloc-types 0.3.1; gpu-descriptor 0.3.2; gpu-descriptor-types 0.2.0; libredox 0.1.25; metal 0.31.0; naga 24.0.0; ndk 0.8.0; ndk 0.9.0; objc2-app-kit 0.2.2; objc2-app-kit 0.3.2; objc2-cloud-kit 0.2.2; objc2-core-data 0.2.2; objc2-core-foundation 0.3.2; objc2-core-graphics 0.3.2; objc2-foundation 0.2.2; objc2-foundation 0.3.2; objc2-io-surface 0.3.2; objc2-metal 0.2.2; objc2-quartz-core 0.2.2; objc2-ui-kit 0.2.2; objc2-user-notifications 0.2.2; png 0.18.1; redox_syscall 0.5.18; redox_syscall 0.9.4; rustix 0.38.44; rustix 1.1.5; rustybuzz 0.20.1; smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0; spirv 0.3.0+sdk-1.3.268.0; wayland-client 0.31.15; wayland-csd-frame 0.3.0; wayland-protocols 0.32.13; wayland-protocols-experimental 20250721.0.1; wayland-protocols-misc 0.3.12; wayland-protocols-plasma 0.3.12; wayland-protocols-wlr 0.3.12; wgpu 24.0.5; wgpu-core 24.0.5; wgpu-hal 24.0.4; wgpu-types 24.0.0; winit 0.30.13; xkbcommon-dl 0.4.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| block | 0.1.6 | metal 0.31.0; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| block-buffer | 0.10.4 | digest 0.10.7 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| block2 | 0.5.1 | objc2-app-kit 0.2.2; objc2-cloud-kit 0.2.2; objc2-contacts 0.2.2; objc2-core-data 0.2.2; objc2-core-image 0.2.2; objc2-core-location 0.2.2; objc2-foundation 0.2.2; objc2-link-presentation 0.2.2; objc2-metal 0.2.2; objc2-quartz-core 0.2.2; objc2-ui-kit 0.2.2; objc2-uniform-type-identifiers 0.2.2; objc2-user-notifications 0.2.2; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| bumpalo | 3.20.3 | wasm-bindgen-macro-support 0.2.129 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| bytemuck | 1.25.2 | ecolor 0.31.1; eframe 0.31.1; egui-wgpu 0.31.1; egui-winit 0.31.1; egui_glow 0.31.1; emath 0.31.1; epaint 0.31.1; image 0.25.10; rgb 0.8.53; rustybuzz 0.20.1; tiny-skia 0.11.4; tiny-skia-path 0.11.4; wgpu-hal 24.0.4; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| bytemuck_derive | 1.12.1 | bytemuck 1.25.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| byteorder-lite | 0.1.0 | image 0.25.10; image-webp 0.2.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| bytes | 1.12.1 | combine 4.6.8 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| calloop | 0.13.0 | calloop-wayland-source 0.3.0; smithay-client-toolkit 0.19.2; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| calloop | 0.14.5 | calloop-wayland-source 0.4.1; smithay-client-toolkit 0.20.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| calloop-wayland-source | 0.3.0 | smithay-client-toolkit 0.19.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| calloop-wayland-source | 0.4.1 | smithay-client-toolkit 0.20.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| cc | 1.8.0 | android-activity 0.6.1; oboe-sys 0.6.1; wayland-backend 0.3.17 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| cesu8 | 1.1.0 | jni 0.21.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| cexpr | 0.6.0 | bindgen 0.72.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| cfg-if | 1.0.5 | ahash 0.8.12; alsa 0.9.1; crc32fast 1.5.2; getrandom 0.3.4; getrandom 0.4.3; half 2.7.1; jni 0.21.1; jni 0.22.4; js-sys 0.3.106; libloading 0.8.9; parking_lot_core 0.9.12; polling 3.11.0; sha2 0.10.9; tiny-skia 0.11.4; wasm-bindgen 0.2.129 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| cfg_aliases | 0.2.2 | glutin 0.32.3; glutin-winit 0.5.0; naga 24.0.0; wgpu 24.0.5; wgpu-core 24.0.5; wgpu-hal 24.0.4; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| cgl | 0.3.2 | glutin 0.32.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| clang-sys | 1.9.1 | bindgen 0.72.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| clipboard-win | 5.4.1 | arboard 3.6.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| codespan-reporting | 0.11.1 | naga 24.0.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| color_quant | 1.1.0 | gif 0.13.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| combine | 4.6.8 | jni 0.21.1; jni 0.22.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| concurrent-queue | 2.5.0 | polling 3.11.0; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| core-foundation | 0.9.4 | core-graphics 0.23.2; core-graphics-types 0.1.3; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| core-foundation-sys | 0.8.7 | core-foundation 0.9.4; coreaudio-rs 0.11.3; cpal 0.15.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| core-graphics | 0.23.2 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| core-graphics-types | 0.1.3 | core-graphics 0.23.2; metal 0.31.0; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| core_maths | 0.1.1 | rustybuzz 0.20.1; ttf-parser 0.25.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| coreaudio-rs | 0.11.3 | cpal 0.15.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| coreaudio-sys | 0.2.18 | coreaudio-rs 0.11.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| cpal | 0.15.3 | rodio 0.20.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| cpufeatures | 0.2.17 | sha2 0.10.9 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| crc32fast | 1.5.2 | flate2 1.1.10; png 0.17.16; png 0.18.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| crossbeam-utils | 0.8.23 | concurrent-queue 2.5.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| crunchy | 0.2.4 | half 2.7.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| crypto-common | 0.1.7 | digest 0.10.7 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| cursor-icon | 1.2.0 | smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0; wayland-csd-frame 0.3.0; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| dasp_sample | 0.11.0 | cpal 0.15.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| data-url | 0.3.2 | usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| digest | 0.10.7 | sha2 0.10.9 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| dispatch | 0.2.0 | objc2-foundation 0.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| dispatch2 | 0.3.1 | glutin 0.32.3; objc2-core-foundation 0.3.2; objc2-core-graphics 0.3.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| displaydoc | 0.2.7 | icu_collections 2.3.0; icu_locale_core 2.3.0; icu_properties 2.3.0; icu_provider 2.3.1; tinystr 0.8.4; zerotrie 0.2.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| dlib | 0.5.3 | wayland-sys 0.31.11; xkbcommon-dl 0.4.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| document-features | 0.2.12 | eframe 0.31.1; egui-wgpu 0.31.1; wgpu 24.0.5; wgpu-core 24.0.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| downcast-rs | 1.2.1 | wayland-backend 0.3.17 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| dpi | 0.1.2 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| ecolor | 0.31.1 | epaint 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| eframe | 0.31.1 | studio-desktop 0.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| egui | 0.31.1 | eframe 0.31.1; egui-wgpu 0.31.1; egui-winit 0.31.1; egui_glow 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| egui-wgpu | 0.31.1 | eframe 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| egui-winit | 0.31.1 | eframe 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| egui_glow | 0.31.1 | eframe 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| either | 1.19.0 | itertools 0.13.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| emath | 0.31.1 | ecolor 0.31.1; egui 0.31.1; epaint 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| epaint | 0.31.1 | egui 0.31.1; egui-wgpu 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| epaint_default_fonts | 0.31.1 | epaint 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| equivalent | 1.0.3 | indexmap 2.14.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| errno | 0.3.14 | rustix 0.38.44; rustix 1.1.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| error-code | 3.4.0 | clipboard-win 5.4.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| euclid | 0.22.14 | kurbo 0.11.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| fax | 0.2.7 | tiff 0.11.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| fdeflate | 0.3.7 | png 0.17.16; png 0.18.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| find-msvc-tools | 0.1.14 | cc 1.8.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| flate2 | 1.1.10 | motion-lipsync 0.1.0; png 0.17.16; png 0.18.1; tiff 0.11.3; usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| float-cmp | 0.9.0 | strict-num 0.1.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| foldhash | 0.1.5 | hashbrown 0.15.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| fontdb | 0.23.0 | usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| foreign-types | 0.5.0 | core-graphics 0.23.2; metal 0.31.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| foreign-types-macros | 0.2.4 | foreign-types 0.5.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| foreign-types-shared | 0.3.1 | foreign-types 0.5.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| form_urlencoded | 1.2.2 | url 2.5.8 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| futures-core | 0.3.34 | futures-util 0.3.34 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| futures-task | 0.3.34 | futures-util 0.3.34 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| futures-util | 0.3.34 | js-sys 0.3.106 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| generic-array | 0.14.7 | block-buffer 0.10.4; crypto-common 0.1.7 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| gethostname | 1.1.0 | x11rb 0.13.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| getrandom | 0.3.4 | ahash 0.8.12 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| getrandom | 0.4.3 | jobserver 0.1.35 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| gif | 0.13.3 | resvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| gl_generator | 0.14.0 | glutin_egl_sys 0.7.1; glutin_glx_sys 0.6.1; glutin_wgl_sys 0.6.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| glob | 0.3.4 | clang-sys 1.9.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| glow | 0.16.0 | eframe 0.31.1; egui_glow 0.31.1; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| glutin | 0.32.3 | eframe 0.31.1; glutin-winit 0.5.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| glutin-winit | 0.5.0 | eframe 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| glutin_egl_sys | 0.7.1 | glutin 0.32.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| glutin_glx_sys | 0.6.1 | glutin 0.32.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| glutin_wgl_sys | 0.6.1 | glutin 0.32.3; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| gpu-alloc | 0.6.2 | wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| gpu-alloc-types | 0.3.1 | gpu-alloc 0.6.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| gpu-descriptor | 0.3.2 | wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| gpu-descriptor-types | 0.2.0 | gpu-descriptor 0.3.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| half | 2.7.1 | tiff 0.11.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| hashbrown | 0.15.5 | gpu-descriptor 0.3.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| hashbrown | 0.17.1 | indexmap 2.14.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| heck | 0.5.0 | strum_macros 0.26.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| hermit-abi | 0.5.3 | polling 3.11.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| hexf-parse | 0.2.1 | naga 24.0.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| hound | 3.5.1 | motion-audio 0.1.0; motion-tts 0.1.0; motion-voice 0.1.0; studio-desktop 0.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| icu_collections | 2.3.0 | icu_normalizer 2.3.0; icu_properties 2.3.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| icu_locale_core | 2.3.0 | icu_properties 2.3.0; icu_provider 2.3.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| icu_normalizer | 2.3.0 | idna_adapter 1.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| icu_normalizer_data | 2.3.0 | icu_normalizer 2.3.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| icu_properties | 2.3.0 | icu_normalizer 2.3.0; idna_adapter 1.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| icu_properties_data | 2.3.0 | icu_properties 2.3.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| icu_provider | 2.3.1 | icu_normalizer 2.3.0; icu_properties 2.3.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| idna | 1.1.0 | url 2.5.8 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| idna_adapter | 1.2.2 | idna 1.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| image | 0.25.10 | arboard 3.6.1; eframe 0.31.1; motion-art 0.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| image-webp | 0.2.4 | resvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| imagesize | 0.13.0 | usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| indexmap | 2.14.2 | naga 24.0.0; serde_yaml 0.9.34+deprecated; toml_edit 0.25.17+spec-1.1.0; wgpu-core 24.0.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| itertools | 0.13.0 | bindgen 0.72.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| itoa | 1.0.18 | serde_json 1.0.154; serde_yaml 0.9.34+deprecated | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| jni | 0.21.1 | cpal 0.15.3; oboe 0.6.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| jni | 0.22.4 | android-activity 0.6.1; webbrowser 1.2.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| jni-macros | 0.22.4 | jni 0.22.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| jni-sys | 0.3.1 | jni 0.21.1; ndk 0.8.0; ndk 0.9.0; ndk-sys 0.5.0+25.2.9519653; ndk-sys 0.6.0+11769913 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| jni-sys | 0.4.1 | jni 0.22.4; jni-sys 0.3.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| jni-sys-macros | 0.4.1 | jni-sys 0.4.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| jobserver | 0.1.35 | cc 1.8.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| js-sys | 0.3.106 | cpal 0.15.3; eframe 0.31.1; glow 0.16.0; wasm-bindgen-futures 0.4.79; web-sys 0.3.106; web-time 1.1.0; wgpu 24.0.5; wgpu-hal 24.0.4; wgpu-types 24.0.0; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| khronos-egl | 6.0.0 | wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| khronos_api | 3.1.0 | gl_generator 0.14.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| kurbo | 0.11.3 | svgtypes 0.15.3; usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| libc | 0.2.190 | alsa 0.9.1; alsa-sys 0.3.1; android-activity 0.6.1; android_system_properties 0.1.6; cc 1.8.0; cgl 0.3.2; clang-sys 1.9.1; core-foundation 0.9.4; core-graphics 0.23.2; core-graphics-types 0.1.3; cpal 0.15.3; cpufeatures 0.2.17; errno 0.3.14; getrandom 0.3.4; getrandom 0.4.3; jobserver 0.1.35; khronos-egl 6.0.0; libredox 0.1.25; mach2 0.4.3; malloc_buf 0.0.6; memmap2 0.9.11; objc2-app-kit 0.2.2; objc2-foundation 0.2.2; orbclient 0.3.55; parking_lot_core 0.9.12; rustix 0.38.44; rustix 1.1.5; smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0; smithay-clipboard 0.7.3; wgpu-hal 24.0.4; winit 0.30.13; x11-dl 2.21.0; x11rb 0.13.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| libloading | 0.8.9 | ash 0.38.0+1.3.281; clang-sys 1.9.1; dlib 0.5.3; glutin 0.32.3; khronos-egl 6.0.0; wgpu-hal 24.0.4; x11rb 0.13.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| libm | 0.2.16 | core_maths 0.1.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| libredox | 0.1.25 | orbclient 0.3.55 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| linux-raw-sys | 0.12.1 | rustix 1.1.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| linux-raw-sys | 0.4.15 | rustix 0.38.44 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| litemap | 0.8.3 | icu_locale_core 2.3.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| litrs | 1.0.1 | document-features 0.2.12 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| lock_api | 0.4.14 | parking_lot 0.12.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| log | 0.4.34 | android-activity 0.6.1; arboard 3.6.1; calloop 0.13.0; eframe 0.31.1; egui 0.31.1; egui-wgpu 0.31.1; egui-winit 0.31.1; egui_glow 0.31.1; epaint 0.31.1; fontdb 0.23.0; gl_generator 0.14.0; jni 0.21.1; jni 0.22.4; metal 0.31.0; naga 24.0.0; ndk 0.8.0; ndk 0.9.0; resvg 0.45.1; rustybuzz 0.20.1; simplecss 0.2.2; smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0; tiny-skia 0.11.4; tracing 0.1.44; usvg 0.45.1; wayland-sys 0.31.11; webbrowser 1.2.4; wgpu 24.0.5; wgpu-core 24.0.5; wgpu-hal 24.0.4; wgpu-types 24.0.0; xkbcommon-dl 0.4.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| mach2 | 0.4.3 | cpal 0.15.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| malloc_buf | 0.0.6 | objc 0.2.7 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| memchr | 2.8.3 | aho-corasick 1.1.5; combine 4.6.8; nom 7.1.3; quick-xml 0.41.0; regex 1.13.1; regex-automata 0.4.18; serde_json 1.0.154; winnow 1.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| memmap2 | 0.9.11 | smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| memoffset | 0.9.1 | egui_glow 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| metal | 0.31.0 | wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| minimal-lexical | 0.2.1 | nom 7.1.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| miniz_oxide | 0.8.9 | png 0.17.16; png 0.18.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| miniz_oxide | 0.9.1 | flate2 1.1.10 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| motion-animation | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| motion-art | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| motion-audio | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| motion-export | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| motion-jobs | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| motion-lipsync | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| motion-project | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| motion-render | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| motion-rig | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| motion-scene | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| motion-script | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| motion-tts | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| motion-voice | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| moxcms | 0.8.1 | image 0.25.10 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| naga | 24.0.0 | wgpu-core 24.0.5; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| ndk | 0.8.0 | cpal 0.15.3; oboe 0.6.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| ndk | 0.9.0 | android-activity 0.6.1; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| ndk-context | 0.1.1 | android-activity 0.6.1; cpal 0.15.3; oboe 0.6.1; webbrowser 1.2.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| ndk-sys | 0.5.0+25.2.9519653 | ndk 0.8.0; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| ndk-sys | 0.6.0+11769913 | android-activity 0.6.1; ndk 0.9.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| nohash-hasher | 0.2.0 | egui 0.31.1; epaint 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| nom | 7.1.3 | cexpr 0.6.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| num-derive | 0.4.2 | oboe 0.6.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| num-traits | 0.2.19 | euclid 0.22.14; image 0.25.10; moxcms 0.8.1; oboe 0.6.1; ordered-float 4.6.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| num_enum | 0.7.6 | android-activity 0.6.1; ndk 0.8.0; ndk 0.9.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| num_enum_derive | 0.7.6 | num_enum 0.7.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc | 0.2.7 | metal 0.31.0; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc-sys | 0.3.5 | objc2 0.5.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2 | 0.5.3 | block2 0.5.1; eframe 0.31.1; objc2-app-kit 0.2.2; objc2-cloud-kit 0.2.2; objc2-contacts 0.2.2; objc2-core-data 0.2.2; objc2-core-image 0.2.2; objc2-core-location 0.2.2; objc2-foundation 0.2.2; objc2-link-presentation 0.2.2; objc2-metal 0.2.2; objc2-quartz-core 0.2.2; objc2-symbols 0.2.2; objc2-ui-kit 0.2.2; objc2-uniform-type-identifiers 0.2.2; objc2-user-notifications 0.2.2; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2 | 0.6.5 | arboard 3.6.1; dispatch2 0.3.1; glutin 0.32.3; objc2-app-kit 0.3.2; objc2-core-foundation 0.3.2; objc2-core-graphics 0.3.2; objc2-foundation 0.3.2; objc2-io-surface 0.3.2; webbrowser 1.2.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-app-kit | 0.2.2 | eframe 0.31.1; objc2-link-presentation 0.2.2; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-app-kit | 0.3.2 | arboard 3.6.1; glutin 0.32.3; webbrowser 1.2.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-cloud-kit | 0.2.2 | objc2-ui-kit 0.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-contacts | 0.2.2 | objc2-core-location 0.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-core-data | 0.2.2 | objc2-app-kit 0.2.2; objc2-ui-kit 0.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-core-foundation | 0.3.2 | arboard 3.6.1; glutin 0.32.3; objc2-app-kit 0.3.2; objc2-core-graphics 0.3.2; objc2-foundation 0.3.2; objc2-io-surface 0.3.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-core-graphics | 0.3.2 | arboard 3.6.1; objc2-app-kit 0.3.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-core-image | 0.2.2 | objc2-app-kit 0.2.2; objc2-ui-kit 0.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-core-location | 0.2.2 | objc2-cloud-kit 0.2.2; objc2-ui-kit 0.2.2; objc2-user-notifications 0.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-encode | 4.1.0 | objc2 0.5.3; objc2 0.6.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-foundation | 0.2.2 | eframe 0.31.1; objc2-app-kit 0.2.2; objc2-cloud-kit 0.2.2; objc2-contacts 0.2.2; objc2-core-data 0.2.2; objc2-core-image 0.2.2; objc2-core-location 0.2.2; objc2-link-presentation 0.2.2; objc2-metal 0.2.2; objc2-quartz-core 0.2.2; objc2-symbols 0.2.2; objc2-ui-kit 0.2.2; objc2-uniform-type-identifiers 0.2.2; objc2-user-notifications 0.2.2; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-foundation | 0.3.2 | arboard 3.6.1; glutin 0.32.3; objc2-app-kit 0.3.2; webbrowser 1.2.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-io-surface | 0.3.2 | objc2-core-graphics 0.3.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-link-presentation | 0.2.2 | objc2-ui-kit 0.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-metal | 0.2.2 | objc2-core-image 0.2.2; objc2-quartz-core 0.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-quartz-core | 0.2.2 | objc2-app-kit 0.2.2; objc2-ui-kit 0.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-symbols | 0.2.2 | objc2-ui-kit 0.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-ui-kit | 0.2.2 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-uniform-type-identifiers | 0.2.2 | objc2-ui-kit 0.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| objc2-user-notifications | 0.2.2 | objc2-ui-kit 0.2.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| oboe | 0.6.1 | cpal 0.15.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| oboe-sys | 0.6.1 | oboe 0.6.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| once_cell | 1.21.4 | ahash 0.8.12; glutin 0.32.3; wasm-bindgen 0.2.129; wayland-sys 0.31.11; wgpu-core 24.0.5; wgpu-hal 24.0.4; x11-dl 2.21.0; x11rb 0.13.2; xkbcommon-dl 0.4.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| orbclient | 0.3.55 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| ordered-float | 4.6.0 | wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| owned_ttf_parser | 0.25.1 | ab_glyph 0.2.32 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| parking_lot | 0.12.5 | arboard 3.6.1; eframe 0.31.1; epaint 0.31.1; wgpu 24.0.5; wgpu-core 24.0.5; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| parking_lot_core | 0.9.12 | parking_lot 0.12.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| paste | 1.0.15 | metal 0.31.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| percent-encoding | 2.3.2 | arboard 3.6.1; eframe 0.31.1; form_urlencoded 1.2.2; url 2.5.8; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| pico-args | 0.5.0 | resvg 0.45.1; usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| pin-project | 1.1.13 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| pin-project-internal | 1.1.13 | pin-project 1.1.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| pin-project-lite | 0.2.17 | futures-util 0.3.34; polling 3.11.0; tokio 1.53.3; tracing 0.1.44 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| pkg-config | 0.3.34 | alsa-sys 0.3.1; khronos-egl 6.0.0; wayland-sys 0.31.11; x11-dl 2.21.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| plain | 0.2.3 | libredox 0.1.25 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| png | 0.17.16 | tiny-skia 0.11.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| png | 0.18.1 | image 0.25.10 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| polling | 3.11.0 | calloop 0.13.0; calloop 0.14.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| potential_utf | 0.1.6 | icu_collections 2.3.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| proc-macro-crate | 3.5.0 | num_enum_derive 0.7.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| proc-macro2 | 1.0.107 | bindgen 0.72.1; bytemuck_derive 1.12.1; displaydoc 0.2.7; foreign-types-macros 0.2.4; jni-macros 0.22.4; num-derive 0.4.2; num_enum_derive 0.7.6; pin-project-internal 1.1.13; quote 1.0.47; serde_derive 1.0.229; strum_macros 0.26.4; syn 2.0.119; syn 3.0.9; synstructure 0.14.0; thiserror-impl 1.0.69; thiserror-impl 2.0.21; wasm-bindgen-macro-support 0.2.129; wayland-scanner 0.31.11; windows-implement 0.58.0; windows-interface 0.58.0; yoke-derive 0.8.4; zerocopy-derive 0.8.63; zerofrom-derive 0.1.8; zerovec-derive 0.11.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| profiling | 1.0.18 | eframe 0.31.1; egui 0.31.1; egui-wgpu 0.31.1; egui-winit 0.31.1; egui_glow 0.31.1; epaint 0.31.1; wgpu 24.0.5; wgpu-core 24.0.5; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| pxfm | 0.1.30 | moxcms 0.8.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| quick-error | 2.0.1 | image-webp 0.2.4; tiff 0.11.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| quick-xml | 0.41.0 | wayland-scanner 0.31.11 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| quote | 1.0.47 | bindgen 0.72.1; bytemuck_derive 1.12.1; displaydoc 0.2.7; foreign-types-macros 0.2.4; jni-macros 0.22.4; jni-sys-macros 0.4.1; num-derive 0.4.2; num_enum_derive 0.7.6; pin-project-internal 1.1.13; serde_derive 1.0.229; strum_macros 0.26.4; syn 2.0.119; syn 3.0.9; synstructure 0.14.0; thiserror-impl 1.0.69; thiserror-impl 2.0.21; wasm-bindgen-macro 0.2.129; wasm-bindgen-macro-support 0.2.129; wayland-scanner 0.31.11; windows-implement 0.58.0; windows-interface 0.58.0; yoke-derive 0.8.4; zerocopy-derive 0.8.63; zerofrom-derive 0.1.8; zerovec-derive 0.11.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| r-efi | 5.3.0 | getrandom 0.3.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| r-efi | 6.0.0 | getrandom 0.4.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| raw-window-handle | 0.6.2 | eframe 0.31.1; egui-winit 0.31.1; glutin 0.32.3; glutin-winit 0.5.0; ndk 0.9.0; wgpu 24.0.5; wgpu-core 24.0.5; wgpu-hal 24.0.4; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| redox_syscall | 0.4.1 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| redox_syscall | 0.5.18 | parking_lot_core 0.9.12 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| redox_syscall | 0.9.4 | libredox 0.1.25 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| regex | 1.13.1 | bindgen 0.72.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| regex-automata | 0.4.18 | regex 1.13.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| regex-syntax | 0.8.11 | regex 1.13.1; regex-automata 0.4.18 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| renderdoc-sys | 1.1.0 | wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| resvg | 0.45.1 | motion-export 0.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| rgb | 0.8.53 | resvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| rodio | 0.20.1 | studio-desktop 0.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| roxmltree | 0.20.0 | usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| rustc-hash | 1.1.0 | naga 24.0.0; wgpu-core 24.0.5; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| rustc-hash | 2.1.3 | bindgen 0.72.1; type-map 0.5.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| rustc_version | 0.4.1 | jni-macros 0.22.4; simd_cesu8 1.2.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| rustix | 0.38.44 | calloop 0.13.0; calloop-wayland-source 0.3.0; smithay-client-toolkit 0.19.2; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| rustix | 1.1.5 | calloop 0.14.5; calloop-wayland-source 0.4.1; gethostname 1.1.0; polling 3.11.0; smithay-client-toolkit 0.20.0; wayland-backend 0.3.17; wayland-client 0.31.15; wayland-cursor 0.31.14; x11rb 0.13.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| rustversion | 1.0.23 | num_enum 0.7.6; strum_macros 0.26.4; wasm-bindgen 0.2.129 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| rustybuzz | 0.20.1 | motion-render 0.1.0; usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| ryu | 1.0.23 | serde_yaml 0.9.34+deprecated | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| same-file | 1.0.6 | walkdir 2.5.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| scoped-tls | 1.0.1 | wayland-backend 0.3.17 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| scopeguard | 1.2.0 | lock_api 0.4.14 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| semver | 1.0.28 | rustc_version 0.4.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| serde | 1.0.229 | motion-animation 0.1.0; motion-art 0.1.0; motion-audio 0.1.0; motion-export 0.1.0; motion-lipsync 0.1.0; motion-project 0.1.0; motion-rig 0.1.0; motion-scene 0.1.0; motion-script 0.1.0; motion-tts 0.1.0; motion-voice 0.1.0; serde_json 1.0.154; serde_yaml 0.9.34+deprecated; smol_str 0.2.2; studio-desktop 0.1.0; url 2.5.8 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| serde_core | 1.0.229 | bitflags 2.13.2; serde 1.0.229; serde_json 1.0.154; toml_datetime 1.1.2+spec-1.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| serde_derive | 1.0.229 | serde 1.0.229; serde_core 1.0.229 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| serde_json | 1.0.154 | motion-art 0.1.0; motion-export 0.1.0; motion-lipsync 0.1.0; motion-project 0.1.0; motion-render 0.1.0; motion-scene 0.1.0; motion-tts 0.1.0; motion-voice 0.1.0; studio-desktop 0.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| serde_yaml | 0.9.34+deprecated | motion-script 0.1.0; studio-desktop 0.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| sha2 | 0.10.9 | motion-lipsync 0.1.0; motion-render 0.1.0; motion-tts 0.1.0; studio-desktop 0.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| shlex | 1.3.0 | bindgen 0.72.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| shlex | 2.0.1 | cc 1.8.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| simd-adler32 | 0.3.10 | fdeflate 0.3.7; miniz_oxide 0.8.9; miniz_oxide 0.9.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| simd_cesu8 | 1.2.0 | jni 0.22.4; jni-macros 0.22.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| simdutf8 | 0.1.5 | simd_cesu8 1.2.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| simplecss | 0.2.2 | usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| siphasher | 1.0.4 | svgtypes 0.15.3; usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| slab | 0.4.12 | calloop 0.13.0; futures-util 0.3.34 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| slotmap | 1.1.1 | fontdb 0.23.0; glow 0.16.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| smallvec | 1.16.3 | icu_normalizer 2.3.0; idna 1.1.0; kurbo 0.11.3; parking_lot_core 0.9.12; rustybuzz 0.20.1; wayland-backend 0.3.17; wgpu 24.0.5; wgpu-core 24.0.5; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| smithay-client-toolkit | 0.19.2 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| smithay-client-toolkit | 0.20.0 | smithay-clipboard 0.7.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| smithay-clipboard | 0.7.3 | egui-winit 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| smol_str | 0.2.2 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| spirv | 0.3.0+sdk-1.3.268.0 | naga 24.0.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| stable_deref_trait | 1.2.1 | yoke 0.8.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| static_assertions | 1.1.0 | eframe 0.31.1; wgpu 24.0.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| strict-num | 0.1.1 | tiny-skia-path 0.11.4; usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| strum | 0.26.3 | naga 24.0.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| strum_macros | 0.26.4 | strum 0.26.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| studio-desktop | 0.1.0 | Oma workspace-moduuli | Oma MIT-ilmoitus; manifesti tarkistettu |
| svgtypes | 0.15.3 | resvg 0.45.1; usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| syn | 2.0.119 | bindgen 0.72.1; jni-macros 0.22.4; jni-sys-macros 0.4.1; num-derive 0.4.2; num_enum_derive 0.7.6; pin-project-internal 1.1.13; strum_macros 0.26.4; thiserror-impl 1.0.69; windows-implement 0.58.0; windows-interface 0.58.0; zerocopy-derive 0.8.63 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| syn | 3.0.9 | bytemuck_derive 1.12.1; displaydoc 0.2.7; foreign-types-macros 0.2.4; serde_derive 1.0.229; synstructure 0.14.0; thiserror-impl 2.0.21; wasm-bindgen-macro-support 0.2.129; yoke-derive 0.8.4; zerofrom-derive 0.1.8; zerovec-derive 0.11.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| synstructure | 0.14.0 | yoke-derive 0.8.4; zerofrom-derive 0.1.8 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| termcolor | 1.4.1 | codespan-reporting 0.11.1; naga 24.0.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| thiserror | 1.0.69 | calloop 0.13.0; egui-wgpu 0.31.1; jni 0.21.1; ndk 0.8.0; ndk 0.9.0; smithay-client-toolkit 0.19.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| thiserror | 2.0.21 | android-activity 0.6.1; jni 0.22.4; motion-animation 0.1.0; motion-art 0.1.0; motion-export 0.1.0; motion-project 0.1.0; motion-render 0.1.0; motion-rig 0.1.0; motion-scene 0.1.0; motion-script 0.1.0; naga 24.0.0; smithay-client-toolkit 0.20.0; wgpu-core 24.0.5; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| thiserror-impl | 1.0.69 | thiserror 1.0.69 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| thiserror-impl | 2.0.21 | thiserror 2.0.21 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| tiff | 0.11.3 | image 0.25.10 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| tiny-skia | 0.11.4 | resvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| tiny-skia-path | 0.11.4 | tiny-skia 0.11.4; usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| tinystr | 0.8.4 | icu_locale_core 2.3.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| tinyvec | 1.13.3 | fontdb 0.23.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| tokio | 1.53.3 | wasm-bindgen-futures 0.4.79 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| toml_datetime | 1.1.2+spec-1.1.0 | toml_edit 0.25.17+spec-1.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| toml_edit | 0.25.17+spec-1.1.0 | proc-macro-crate 3.5.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| toml_parser | 1.1.5+spec-1.1.0 | toml_edit 0.25.17+spec-1.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| tracing | 0.1.44 | calloop 0.14.5; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| tracing-core | 0.1.36 | tracing 0.1.44 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| ttf-parser | 0.25.1 | fontdb 0.23.0; motion-render 0.1.0; owned_ttf_parser 0.25.1; rustybuzz 0.20.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| type-map | 0.5.1 | egui-wgpu 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| typenum | 1.20.1 | crypto-common 0.1.7; generic-array 0.14.7 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| unicode-bidi | 0.3.18 | usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| unicode-bidi-mirroring | 0.4.0 | rustybuzz 0.20.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| unicode-ccc | 0.4.0 | rustybuzz 0.20.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| unicode-ident | 1.0.27 | proc-macro2 1.0.107; syn 2.0.119; syn 3.0.9; wasm-bindgen-shared 0.2.129 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| unicode-properties | 0.1.4 | rustybuzz 0.20.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| unicode-script | 0.5.8 | rustybuzz 0.20.1; usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| unicode-segmentation | 1.13.3 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| unicode-vo | 0.1.0 | usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| unicode-width | 0.1.14 | codespan-reporting 0.11.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| unicode-xid | 0.2.6 | naga 24.0.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| unsafe-libyaml | 0.2.11 | serde_yaml 0.9.34+deprecated | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| url | 2.5.8 | webbrowser 1.2.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| usvg | 0.45.1 | resvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| utf8_iter | 1.0.4 | icu_collections 2.3.0; idna 1.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| version_check | 0.9.5 | ahash 0.8.12; generic-array 0.14.7; slotmap 1.1.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| walkdir | 2.5.0 | jni 0.21.1; jni 0.22.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wasip2 | 1.0.4+wasi-0.2.12 | getrandom 0.3.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wasm-bindgen | 0.2.129 | cpal 0.15.3; eframe 0.31.1; egui_glow 0.31.1; glow 0.16.0; js-sys 0.3.106; wasm-bindgen-futures 0.4.79; web-sys 0.3.106; web-time 1.1.0; wgpu 24.0.5; wgpu-hal 24.0.4; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wasm-bindgen-futures | 0.4.79 | cpal 0.15.3; eframe 0.31.1; wgpu 24.0.5; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wasm-bindgen-macro | 0.2.129 | wasm-bindgen 0.2.129 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wasm-bindgen-macro-support | 0.2.129 | wasm-bindgen-macro 0.2.129 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wasm-bindgen-shared | 0.2.129 | wasm-bindgen 0.2.129; wasm-bindgen-macro-support 0.2.129 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wayland-backend | 0.3.17 | calloop-wayland-source 0.3.0; calloop-wayland-source 0.4.1; smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0; smithay-clipboard 0.7.3; wayland-client 0.31.15; wayland-csd-frame 0.3.0; wayland-protocols 0.32.13; wayland-protocols-experimental 20250721.0.1; wayland-protocols-misc 0.3.12; wayland-protocols-plasma 0.3.12; wayland-protocols-wlr 0.3.12; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wayland-client | 0.31.15 | calloop-wayland-source 0.3.0; calloop-wayland-source 0.4.1; smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0; wayland-cursor 0.31.14; wayland-protocols 0.32.13; wayland-protocols-experimental 20250721.0.1; wayland-protocols-misc 0.3.12; wayland-protocols-plasma 0.3.12; wayland-protocols-wlr 0.3.12; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wayland-csd-frame | 0.3.0 | smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wayland-cursor | 0.31.14 | smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wayland-protocols | 0.32.13 | smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0; wayland-protocols-experimental 20250721.0.1; wayland-protocols-misc 0.3.12; wayland-protocols-plasma 0.3.12; wayland-protocols-wlr 0.3.12; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wayland-protocols-experimental | 20250721.0.1 | smithay-client-toolkit 0.20.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wayland-protocols-misc | 0.3.12 | smithay-client-toolkit 0.20.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wayland-protocols-plasma | 0.3.12 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wayland-protocols-wlr | 0.3.12 | smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wayland-scanner | 0.31.11 | smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0; wayland-client 0.31.15; wayland-protocols 0.32.13; wayland-protocols-experimental 20250721.0.1; wayland-protocols-misc 0.3.12; wayland-protocols-plasma 0.3.12; wayland-protocols-wlr 0.3.12 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wayland-sys | 0.31.11 | glutin 0.32.3; wayland-backend 0.3.17 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| web-sys | 0.3.106 | cpal 0.15.3; eframe 0.31.1; egui_glow 0.31.1; glow 0.16.0; webbrowser 1.2.4; wgpu 24.0.5; wgpu-hal 24.0.4; wgpu-types 24.0.0; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| web-time | 1.1.0 | eframe 0.31.1; egui-wgpu 0.31.1; egui-winit 0.31.1; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| webbrowser | 1.2.4 | egui-winit 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| weezl | 0.1.12 | gif 0.13.3; tiff 0.11.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wgpu | 24.0.5 | egui-wgpu 0.31.1; motion-render 0.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wgpu-core | 24.0.5 | wgpu 24.0.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wgpu-hal | 24.0.4 | wgpu 24.0.5; wgpu-core 24.0.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wgpu-types | 24.0.0 | wgpu 24.0.5; wgpu-core 24.0.5; wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| winapi | 0.3.9 | eframe 0.31.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| winapi-i686-pc-windows-gnu | 0.4.0 | winapi 0.3.9 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| winapi-util | 0.1.11 | same-file 1.0.6; termcolor 1.4.1; walkdir 2.5.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | winapi 0.3.9 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows | 0.54.0 | cpal 0.15.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows | 0.58.0 | wgpu-hal 24.0.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-core | 0.54.0 | windows 0.54.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-core | 0.58.0 | windows 0.58.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-implement | 0.58.0 | windows-core 0.58.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-interface | 0.58.0 | windows-core 0.58.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-link | 0.2.1 | gethostname 1.1.0; jni 0.22.4; libloading 0.8.9; parking_lot_core 0.9.12; windows-sys 0.61.2; windows-targets 0.53.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-result | 0.1.2 | windows-core 0.54.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-result | 0.2.0 | windows-core 0.58.0; windows-strings 0.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-strings | 0.1.0 | windows-core 0.58.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-sys | 0.45.0 | jni 0.21.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-sys | 0.52.0 | glutin 0.32.3; glutin_egl_sys 0.7.1; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-sys | 0.59.0 | eframe 0.31.1; rustix 0.38.44 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-sys | 0.60.2 | arboard 3.6.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-sys | 0.61.2 | errno 0.3.14; polling 3.11.0; rustix 1.1.5; winapi-util 0.1.11 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-targets | 0.42.2 | windows-sys 0.45.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-targets | 0.52.6 | windows 0.54.0; windows 0.58.0; windows-core 0.54.0; windows-core 0.58.0; windows-result 0.1.2; windows-result 0.2.0; windows-strings 0.1.0; windows-sys 0.52.0; windows-sys 0.59.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows-targets | 0.53.5 | windows-sys 0.60.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_aarch64_gnullvm | 0.42.2 | windows-targets 0.42.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_aarch64_gnullvm | 0.52.6 | windows-targets 0.52.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_aarch64_gnullvm | 0.53.1 | windows-targets 0.53.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_aarch64_msvc | 0.42.2 | windows-targets 0.42.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_aarch64_msvc | 0.52.6 | windows-targets 0.52.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_aarch64_msvc | 0.53.1 | windows-targets 0.53.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_i686_gnu | 0.42.2 | windows-targets 0.42.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_i686_gnu | 0.52.6 | windows-targets 0.52.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_i686_gnu | 0.53.1 | windows-targets 0.53.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_i686_gnullvm | 0.52.6 | windows-targets 0.52.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_i686_gnullvm | 0.53.1 | windows-targets 0.53.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_i686_msvc | 0.42.2 | windows-targets 0.42.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_i686_msvc | 0.52.6 | windows-targets 0.52.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_i686_msvc | 0.53.1 | windows-targets 0.53.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_x86_64_gnu | 0.42.2 | windows-targets 0.42.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_x86_64_gnu | 0.52.6 | windows-targets 0.52.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_x86_64_gnu | 0.53.1 | windows-targets 0.53.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_x86_64_gnullvm | 0.42.2 | windows-targets 0.42.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_x86_64_gnullvm | 0.52.6 | windows-targets 0.52.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_x86_64_gnullvm | 0.53.1 | windows-targets 0.53.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_x86_64_msvc | 0.42.2 | windows-targets 0.42.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_x86_64_msvc | 0.52.6 | windows-targets 0.52.6 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| windows_x86_64_msvc | 0.53.1 | windows-targets 0.53.5 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| winit | 0.30.13 | eframe 0.31.1; egui-wgpu 0.31.1; egui-winit 0.31.1; egui_glow 0.31.1; glutin-winit 0.5.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| winnow | 1.0.4 | toml_edit 0.25.17+spec-1.1.0; toml_parser 1.1.5+spec-1.1.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| wit-bindgen | 0.57.1 | wasip2 1.0.4+wasi-0.2.12 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| writeable | 0.6.4 | icu_locale_core 2.3.0; icu_provider 2.3.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| x11-dl | 2.21.0 | glutin 0.32.3; glutin_glx_sys 0.6.1; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| x11rb | 0.13.2 | arboard 3.6.1; winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| x11rb-protocol | 0.13.2 | x11rb 0.13.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| xcursor | 0.3.11 | wayland-cursor 0.31.14 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| xkbcommon-dl | 0.4.2 | winit 0.30.13 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| xkeysym | 0.2.1 | smithay-client-toolkit 0.19.2; smithay-client-toolkit 0.20.0; xkbcommon-dl 0.4.2 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| xml-rs | 0.8.29 | gl_generator 0.14.0 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| xmlwriter | 0.1.0 | usvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| yoke | 0.8.3 | icu_collections 2.3.0; icu_provider 2.3.1; zerotrie 0.2.5; zerovec 0.11.8 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| yoke-derive | 0.8.4 | yoke 0.8.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zerocopy | 0.8.63 | ahash 0.8.12; half 2.7.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zerocopy-derive | 0.8.63 | zerocopy 0.8.63 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zerofrom | 0.1.8 | icu_collections 2.3.0; icu_provider 2.3.1; yoke 0.8.3; zerotrie 0.2.5; zerovec 0.11.8 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zerofrom-derive | 0.1.8 | zerofrom 0.1.8 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zerotrie | 0.2.5 | icu_properties 2.3.0; icu_provider 2.3.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zerovec | 0.11.8 | icu_collections 2.3.0; icu_locale_core 2.3.0; icu_normalizer 2.3.0; icu_properties 2.3.0; icu_provider 2.3.1; potential_utf 0.1.6; tinystr 0.8.4 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zerovec-derive | 0.11.6 | zerovec 0.11.8 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zlib-rs | 0.6.8 | flate2 1.1.10 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zmij | 1.0.23 | serde_json 1.0.154 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zune-core | 0.4.12 | zune-jpeg 0.4.21 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zune-core | 0.5.3 | image 0.25.10; zune-jpeg 0.5.15 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zune-jpeg | 0.4.21 | resvg 0.45.1 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |
| zune-jpeg | 0.5.15 | image 0.25.10; tiff 0.11.3 | Lukitus tarkistettu; lisenssi ja jakelutarve tarkistamatta |

## Python, mallit ja ulkoiset työkalut

| Kohde | Perustelu | Tarkistustila |
| --- | --- | --- |
| Kokoro / Torch | Valinnainen paikallinen englannin puhesynteesi ja mallin suoritus | Manifestipinnit JSON:ssa; transitiivinen wheel-lukitus ja lisenssit avoinna |
| misaki[en] / spaCy / en_core_web_sm | Englannin kielellinen esikäsittely | Manifestipinnit tarkistettu; tarkkojen artefaktien ehdot avoinna |
| NumPy / preshed | Numeeriset taulukot / kieliputken riippuvuus | Kehitysvaatimukset tarkistettu; asennettua ympäristöä ei tutkittu |
| Kokoro config, weights, af_heart, am_michael | Synteesin konfiguraatio, painot ja äänet | Revisiot ja hash-ilmoitukset JSON:ssa; tavuja ja oikeuksia ei tarkistettu |
| Piper ja sen äänimallit | Valinnainen suomen puhe | Käyttäjän valitsema binaari/malli; versio, lisenssi ja aineistokohtaiset ehdot avoinna |
| macOS say | Vaihtoehtoinen järjestelmäpuhe | Toteutus tunnistettu; mallivapaudesta ei näyttöä |
| Rhubarb ja tunnistinaineistot | Valinnainen suun ajoituksen arviointi | Ei sallita tiukkaan mallivapaaseen tilaan ilman erillistä tarkastusta |
| FFmpeg | Videon ja äänen koodaus | Valittu binaari, build-optiot ja lisenssivelvoitteet tarkistamatta |
| Fontit, sanastot, kuvat ja muut tuodut resurssit | Teksti ja kohtauksen aineisto | Pakettikohtainen alkuperä- ja lisenssiluettelo tarvitaan |

Ehdokkaat `broadsheet`, `rust-rule-engine`, `animato`, `rusty_skelform` eivät esiinny Cargo.lockissa. Niitä ei hyväksytä riippuvuuksiksi tässä työssä. Nykyisiä moduuleja käytetään; ehdokkaan lisääminen vaatii API-, ylläpito-, lisenssi- ja yhteensopivuusnäytön.
