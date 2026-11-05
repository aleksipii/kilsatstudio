# Puhe valmiina sovelluksessa (bundled speech)

Koeta Studio.app voidaan paketoida niin, että kaikki puhe- ja suuajoitustyökalut
ovat valmiina sovelluksen sisällä. Käyttäjän ei tarvitse ladata, asentaa tai
yhdistää mitään: sovellus löytää ne käynnistyessään ja toimii ilman verkkoa.

| Työkalu | Käyttö | Sijainti sovelluksessa |
| --- | --- | --- |
| Kokoro-82M + 28 englanninkielistä ääntä | Englanninkielinen puhe | `Contents/Resources/speech/kokoro` |
| Piper 1.8.0 + `fi_FI-harri-medium` (`piper_harri`) | Suomenkielinen puhe | `speech/bin/piper`, `speech/piper` |
| Rhubarb Lip Sync 1.14.0 (PocketSphinx) | Suun ajoitusarvio | `speech/rhubarb` |
| CPython 3.12 (python-build-standalone) | Kokoron ja Piperin ajoympäristö | `speech/python` |
| FFmpeg 9.0.2 + x264 (valinnainen, staattinen) | MP4-, ProRes- ja animaatiovienti | `speech/ffmpeg/bin/ffmpeg` |

## Rakentaminen

Lataukset tehdään vain rakennusvaiheessa ja vain luvalla (`--allow-downloads`).
Jokainen tiedosto tarkistetaan [kiinnitettyä luetteloa](../services/speech-bundle/manifest.json)
vastaan (HTTPS, koko, SHA-256). Kokoron malli- ja äänipinnit tulevat
[setup-manifest.json](../services/kokoro-runtime/setup-manifest.json)- ja
[english-voices.json](../services/kokoro-runtime/english-voices.json)-tiedostoista.

```sh
python3 scripts/build_speech_bundle.py --output /absolute/build/speech \
  --download-cache /absolute/build/speech-cache --allow-downloads
# Valinnainen: FFmpeg mukaan, jolloin vienti ei tarvitse Homebrew'ta.
scripts/build_ffmpeg_bundle.sh /absolute/build/ffmpeg /absolute/build/ffmpeg-cache
python3 scripts/add_ffmpeg_to_speech_bundle.py /absolute/build/speech /absolute/build/ffmpeg
scripts/build_release_app.sh /absolute/build/release /absolute/build/speech
```

`build_ffmpeg_bundle.sh` kääntää FFmpeg 9.0.2:n ja x264:n (Homebrew'n kiinnittämä r3222)
kiinnitetyistä lähdepaketeista (SHA-256) staattisesti. Tulos linkittää vain macOS:n
järjestelmäkirjastoihin, toimii Apple Siliconilla macOS 13:sta alkaen, ja sen
mukana ovat lisenssitekstit ja `SOURCES.txt` (lähteet ja configure-rivit). Kun
paketissa on FFmpeg, sovellus täyttää FFmpeg-polun kaikkiin vienteihin itse.
Tällainen FFmpeg on GPL-2.0-or-later: julkinen jakelu edellyttää lähdekoodin
tarjoamista.

Rakennin:

1. purkaa siirrettävän Pythonin ja asentaa siihen Kokoron pinnatut paketit,
   englannin spaCy-mallin ja `piper-tts==1.8.0` (`--only-binary=:all:`);
2. esikääntää Python-tavukoodin (`unchecked-hash`), jotta allekirjoitetun
   sovelluksen sisään ei koskaan kirjoiteta;
3. kopioi Kokoron mallin, kaikki pinnatut englanninkieliset äänet, Piperin
   suomiäänen ja Rhubarbin (vain ohjelma, `res/` ja lisenssi);
4. tallentaa `evidence/freeze.txt`- ja `evidence/distributions.json`-tiedostot
   (asennetut paketit ja niiden lisenssitiedostot);
5. ajaa Kokoron esitarkistuksen (`doctor.py`) paketin omalla Pythonilla;
6. poistaa symboliset linkit ja allekirjoittaa jokaisen Mach-O-tiedoston ad hoc;
7. kirjoittaa `speech-bundle.json`-kuvauksen suhteellisin poluin.

`package_macos.py --speech-bundle` kopioi hakemiston muuttamattomana ja
`package_audit.py --speech-bundle` vertaa jokaisen paketoidun tiedoston tavuja
rakennettuun hakemistoon. Vain tällöin tarkastuksen kokorajat ovat suuremmat.

## Sovelluksen käynnistys

[`bundled_speech.rs`](../apps/studio-desktop/src/bundled_speech.rs) etsii
`Contents/Resources/speech/speech-bundle.json`-tiedoston (kehitysajossa myös
`KILSAT_SPEECH_BUNDLE=/absolute/speech`). Polut ratkaistaan ja niiden on
pysyttävä paketin sisällä. Kokoron asetukset, Piperin profiili ja Rhubarbin
ohjelmapolku täytetään automaattisesti, ja asennusosio piilotetaan.
Kirjoitettavat tiedostot (välimuistit ja luotu Piper-profiili) ovat kansiossa
`~/Library/Application Support/Kilsat Studio/Bundled speech`.

Tiukka mallivapaa käytäntö (`ModelPolicy::Strict`) estää edelleen kaikki mallit,
vaikka ne olisivat paketissa.

## Rajoitukset ja avoimet asiat

- Rhubarbin macOS 1.14.0 -ohjelma on vain x86_64: Apple Siliconilla se tarvitsee
  Rosetta 2:n. Ilman Rosettaa suuajoitus antaa virheen; puhe toimii silti.
- Sovelluksen koko kasvaa noin 1–1,5 Gt:lla.
- Allekirjoitus on ad hoc. Developer ID -allekirjoitus ja notarisointi vaativat
  paketin Mach-O-tiedostojen uudelleenallekirjoituksen (hardened runtime) ja on
  oma erillinen vaiheensa.
- piper-tts 1.8.0 ja sen sisältämä eSpeak NG ovat GPL-3.0-or-later. Ne ajetaan
  erillisinä prosesseina, mutta julkinen jakelu edellyttää lisenssitekstiä ja
  vastaavan lähdekoodin saatavuutta GPL:n mukaisesti. Päätä tämä ennen julkaisua.
- Kokoron äänitensoreiden ja harri-äänen oikeuksia ei ole katselmoitu
  (ks. [kokoro-dependency-review.md](kokoro-dependency-review.md)). Paketti tallentaa
  todisteet, mutta `distribution_approved` pysyy `false`.
- Pinnat koskevat suoria riippuvuuksia; pip ratkaisee loput, ja todellinen
  ympäristö tallennetaan `freeze.txt`-tiedostoon.
