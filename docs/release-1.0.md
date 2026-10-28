# Kilsat Studio 1.0

Kilsat Studio 1.0 on ensimmäinen versioitu julkaisu macOS-työpöytäsovelluksesta
(Apple Silicon, macOS 13+). Sovellus on nimeltään **Kilsat Studio.app**, versio
1.0.0, tunniste `studio.kilsat.studio`.

## Mitä 1.0 sisältää

- MotionScript-käsikirjoitus → aikajana → kohtaus → esikatselu ja vienti samalla
  näytekellolla (24 kHz).
- Tikku-ukko- ja omat rigit, käteen kiinnitettävät esineet, IK ja jouset,
  motion blocks -paletti ja animaatioperiaatteet.
- Ankkuroitu esinekirjasto (101 esinettä, 18 paikkaa, 18 lavastusta).
- Puhelimen ja tabletin näyttöpaikat: oma kuva, kuvasarja tai video
  (cover/contain/stretch, trim, nopeus, silmukka, napautukset); videoruudut
  puretaan FFmpegillä välimuistiin ja vienti lukee ne ruutu kerrallaan.
- Kokoro-82M englanninkielinen puhe paikallisesti (käyttäjän luvalla ladattava),
  Piper/suomi, tuodut WAV-tiedostot, huulisynkka-arviot.
- MP4-vienti FFmpegillä, tekstitykset, kamera ja valaistus.
- Esimerkkijakso `examples/screen-demo/live-demo.kilsat` ("The Live Demo").

## Rakentaminen

```sh
scripts/build_release_app.sh dist/release-1.0
```

Skripti kääntää release-profiilin `studio-desktop`- ja `gpu_probe`-ohjelmat
sekä `head_pose`-apuohjelman, tarkistaa että vain järjestelmäkirjastoja
linkitetään, paketoi ZIPin (`Kilsat-Studio-1.0.0-arm64.zip`) ja ajaa
`package_audit.py`:n. CI-työnkulku `macos-release-app.yml` tekee saman mainille,
`kilsatstudio1.0`-haaralle ja `v*`-tageille ja tallentaa ZIPin artefaktina.

## Allekirjoitus ja jakelu

Paketti on ad-hoc-allekirjoitettu, ei notarisoitu. Gatekeeper pyytää
ensimmäisellä käynnistyksellä **Avaa silti** -hyväksynnän
([käytännön testiohje](practical-testing.md)). Developer ID -allekirjoitus ja
Applen notarisointi tehdään erikseen `scripts/developer_release.py`:llä tai
`macos-release.yml`-työnkululla, kun Developer ID Application -varmenne ja
notarisointitunnukset ovat käytössä ([ohje](mac-production-release.md)).

Mallipainot, äänet ja Python-ympäristö eivät sisälly pakettiin; Kokoro
asennetaan sovelluksen omalla, luvan kysyvällä asennuksella.

## Avoimet hyväksynnät

Julkaisu ei hyväksy automaattisesti taiteellista laatua, huulisynkan tarkkuutta,
ääni- tai mallilisenssejä jakelua varten eikä fyysisen Macin laitetestiä; ne
kirjataan [laitetestausmenettelyn](workstreams/mac-device-acceptance.md) mukaan.
