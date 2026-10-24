# Kilsat-kauden lähdeprojektit ja paikallinen renderöinti

Kausi on uusi alkuperäinen suomenkielinen satiiri, jossa samaistuttava Aino ja
omasta erinomaisuudestaan vakuuttunut Rane käsittelevät ajamista, työelämää ja
ajopäiväkirjaa. Lähdeaineisto sisältää 31 erikseen kirjoitettua tarinaa,
omat hahmorigit ja kuvituksen. Se ei ole aiemmin kadonneen kauden palautus.

Projektit ovat kansiossa examples/kilsat-season nimillä ep01.kilsat–ep31.kilsat.
[Kausikäsikirjoitus](../examples/kilsat-season/KAUSI.md) sisältää repliikit ja
kohtauskuvaukset. `manifest.json` sitoo lähteet ja resurssit SHA-256-tiivisteisiin;
se kuvaa lähdepaketin, eikä sen not_rendered-tilaa käytetä renderöintilaskurina.
Valmiit videot ja niiden todellinen tila tarkistetaan kunkin ajon kuiteista.
Pelkkä 31 projektitiedoston olemassaolo ei tarkoita, että kaikki 31 MP4:ää olisi
renderöity tai hyväksytty julkaistavaksi.

## Valmiit Full HD -videot

[CI-ajo 40127342644](https://github.com/aleksipii/kilsatstudio/actions/runs/40127342644)
renderöi ja tarkisti kaikki 31 suomenkielistä jaksoa sekä seitsemän koostetta
1920 × 1080 / 24 fps -muodossa. Jaksot kestävät 39,3–45,3 sekuntia, yhteensä
22 minuuttia 8,4 sekuntia. Lähdeversio on 3e847b7348479f2ec6cacc9e690bb209003f2393.
Integraattori latasi kaikki seitsemän pakettia ja tarkisti tiedostotiivisteet;
[todennusraportti](evidence/kilsat-season-fullhd-ci.json) sisältää luvut ja tunnisteet.

| Jaksot | Lataa jaksot ja kooste |
| --- | --- |
| 01–05 | [Full HD -ryhmä 1](https://github.com/aleksipii/kilsatstudio/actions/runs/40127342644/artifacts/12573639619) |
| 06–10 | [Full HD -ryhmä 2](https://github.com/aleksipii/kilsatstudio/actions/runs/40127342644/artifacts/12573074289) |
| 11–15 | [Full HD -ryhmä 3](https://github.com/aleksipii/kilsatstudio/actions/runs/40127342644/artifacts/12573354929) |
| 16–20 | [Full HD -ryhmä 4](https://github.com/aleksipii/kilsatstudio/actions/runs/40127342644/artifacts/12573519599) |
| 21–25 | [Full HD -ryhmä 5](https://github.com/aleksipii/kilsatstudio/actions/runs/40127342644/artifacts/12572994378) |
| 26–30 | [Full HD -ryhmä 6](https://github.com/aleksipii/kilsatstudio/actions/runs/40127342644/artifacts/12572624460) |
| 31 | [Full HD -ryhmä 7](https://github.com/aleksipii/kilsatstudio/actions/runs/40127342644/artifacts/12572629344) |

Pura ZIP ja etsi season-group-alkuinen kansio. Jokaisen ep-alkuisen kansion
sequence.mp4 on kyseinen jakso; compilation/sequence.mp4 on ryhmän kooste.
GitHub-artefaktit säilyvät 14 päivää; lataa säilytettävät videot itsellesi.
Tekninen renderöinti on valmis. Taiteellista laatua, huulisynkkaa ja
julkaisuoikeuksia ei ole hyväksytty; niitä koskevat tarkistukset ovat alempana.

Lähdeprofiili on 1920 × 1080, 24 kuvaa sekunnissa ja 30–60 sekuntia jaksoa kohden.
Lähteen kesto on ajoitusluonnos. Renderöintityökalu tuottaa oikean puheen ja
sovittaa sitä sisältävien otosten kestot mitatun äänen mukaan. Puheen pituutta ei
saa peittää katkaisemalla repliikkejä tai muuttamalla kuvan resoluutiota salaa.

## Seitsemän koosteryhmää

Työkalu ajaa yhden ryhmän kerrallaan. Ensimmäiset kuusi ryhmää sisältävät viisi
jaksoa; seitsemännessä on yksin finaali. Jaksoja ei monisteta viiden täyttämiseksi.

| --group | Jaksot | Paikallinen lopputulos |
| --- | --- | --- |
| 1 | 01–05 | Viisi jaksovideota ja kooste 1 |
| 2 | 06–10 | Viisi jaksovideota ja kooste 2 |
| 3 | 11–15 | Viisi jaksovideota ja kooste 3 |
| 4 | 16–20 | Viisi jaksovideota ja kooste 4 |
| 5 | 21–25 | Viisi jaksovideota ja kooste 5 |
| 6 | 26–30 | Viisi jaksovideota ja kooste 6 |
| 7 | 31 | Finaalijakso ja sen oma kooste |

Koosteet jäävät paikallisiksi tiedostoiksi. Työkalu ei kirjaudu YouTubeen eikä
lataa videoita sinne. Myös youtube_groups-nimi manifestissa tarkoittaa vasta
suunniteltua ryhmittelyä, ei julkaistuja YouTube-videoita.

## Valmistele ohjelmat ja puhe

Tarvitset Python 3:n, paikalliset FFmpeg- ja FFprobe-ohjelmat sekä rakennettua
production_export-vientiohjelmaa vastaavan tämän repositorion version. Rakenna
vientiohjelma repositorion juuresta:

```sh
cargo build --locked --release -p motion-export --example production_export
```

Asenna Piper ja suomenkielinen ääni erikseen ja tee
[suomen puheen ohjeen](finnish-speech.md) mukainen Piper-profiili. Kaikki profiilin
polut ovat absoluuttisia. Kausi käyttää fi-fi-kieltä ja piper_harri-tunnusta;
profiilin preset-tunnuksen on vastattava projektia. Mallia, ääntä tai moottoria ei
ladata renderöintikomennolla automaattisesti. Renderöinnin käynnistäminen aloittaa
paikallisen puhesynteesin, jos valmiiksi varmennettua jaksomediaa ei käytetä uudelleen.

Valitse oma projekti-, ohjelma-, profiili- ja tuloshakemistosi. Seuraavat polut
ovat esimerkkejä, jotka on korvattava todellisilla koko poluilla:

```sh
cd /koko/polku/kilsatstudio
studio_exporter="/koko/polku/kilsatstudio/target/release/examples/production_export"
studio_voice_profile="/koko/polku/piper-profile.json"
studio_ffmpeg="/koko/polku/ffmpeg"
studio_ffprobe="/koko/polku/ffprobe"
studio_outputs="/koko/polku/renderoinnit"
```

Tässä kausiajossa käytetään Piper-profiilia. Macin järjestelmä-ääniä käsitellään
sovelluksessa yksityisen esikatselun vaihtoehtona, eikä niitä hyväksytä tämän
kausityökalun julkaisuääniksi. Piper-profiili ja todellinen WAV eivät itsessään
ratkaise moottorin, mallin, pohjamallin tai ääniaineiston julkaisuoikeuksia.

## Tee ensin 960 × 540 -tarkistusajo

Aja ensimmäinen ryhmä uuteen tuloskansioon:

```sh
python3 scripts/render_kilsat_season.py \
  --season examples/kilsat-season \
  --group 1 \
  --exporter "$studio_exporter" \
  --piper-config "$studio_voice_profile" \
  --ffmpeg "$studio_ffmpeg" \
  --ffprobe "$studio_ffprobe" \
  --review-size 960x540 \
  --output "$studio_outputs/review-group-01"
```

--review-size pienentää rasterin tarkistamista varten säilyttäen kuva-alan
koordinaatit, sommittelun ja 24 kuvan sekuntinopeuden. Myös 1280x720 on sallittu.
Tarkistusajo merkitään review=true. Sen nopeus tai kuvanlaatu ei osoita Full HD
-tulosta. Katso kuvat, kuuntele puhe ja tarkista vitsin ajoitus ennen raskaampia ajoja.

Työkalu pyytää vientiohjelmalta mitattuun puheeseen perustuvan uudelleenajoituksen
sekä muutaman PNG-tarkistuskuvan. Näiden sisäiset liput ovat --retime-dialogue ja
--review-frames; niitä ei lisätä Python-kausityökalun komentoon.

## Renderöi Full HD

Full HD -komennossa jätä --review-size kokonaan pois ja käytä erillistä uutta
kansiota. --review-size 1920x1080 ei ole kausityökalun sallittu valinta.

```sh
python3 scripts/render_kilsat_season.py \
  --season examples/kilsat-season \
  --group 1 \
  --exporter "$studio_exporter" \
  --piper-config "$studio_voice_profile" \
  --ffmpeg "$studio_ffmpeg" \
  --ffprobe "$studio_ffprobe" \
  --output "$studio_outputs/fullhd-group-01"
```

Tämä pyytää 1920 × 1080 -videota 24 kuvan sekuntinopeudella. Yksittäisen jakson
on pysyttävä 30–60 sekunnissa myös oikean puheen jälkeen. Jos kesto ei sovi,
korjaa lähdettä tai tarkoituksellisia taukoja ja tee uusi ajo. Myös Full HD -ajon
mukana voi syntyä PNG-tarkistuskuvia; ne eivät muuta MP4:n resoluutiota.

Vaihda --group-arvoksi 2–7 ja anna kullekin ryhmälle oma kansio. Aja ryhmät aluksi
peräkkäin. Yhden ryhmän enintään viiden jakson valmistus ja yhdistäminen on eri
mittaus kuin yhden jakson vienti. [Suoratoistoviennin rajat](fullhd-performance.md)
ja ajokohtaiset aikaraportit kertovat, mitä todella mitattiin; tähän ohjeeseen ei
sisälly lupausta viidessä minuutissa valmistuvasta kaudesta.

## Jatka vain erillisellä --resume-valinnalla

Olemassa olevaan tuloskansioon kirjoittaminen vaatii --resume-valinnan. Esimerkiksi
saman ensimmäisen tarkistusryhmän tarkistus ja jatkaminen:

```sh
python3 scripts/render_kilsat_season.py \
  --season examples/kilsat-season \
  --group 1 \
  --exporter "$studio_exporter" \
  --piper-config "$studio_voice_profile" \
  --ffmpeg "$studio_ffmpeg" \
  --ffprobe "$studio_ffprobe" \
  --review-size 960x540 \
  --output "$studio_outputs/review-group-01" \
  --resume
```

Kansion oman render-inputs.json-tunnisteen pitää vastata täsmälleen samaa ryhmää,
resoluutiota, lähdemanifestia, profiilia ja ohjelmatiedostoja. Valmiiden jaksojen
MP4-, WAV- ja putkiraporttien tiivisteiden pitää täsmätä kuitteihin; videot
FFprobe-tarkistetaan uudelleen. Muuttuneet syötteet tai media estävät käytön.

--resume ei tarkoita mielivaltaisten vanhojen kansioiden hyväksymistä. Se käyttää
uudelleen vain tunnistetut, valmiit tulokset ja voi jatkaa vielä puuttuviin
jaksoihin. Ilman --resume-valintaa tarvitaan uusi kansio, ja --resume vaatii jo
olemassa olevan oman ajokansion. Tarkistusajoa ei muuteta Full HD -ajoksi saman
kansion sisällä.

Epäonnistunut tai keskeytynyt osajakso tai kooste voi jäädä keskeneräiseksi ilman
hyväksyttyä kuittia. Työkalu ei korvaa sitä automaattisesti. Säilytä tällainen
kansio ja lokit vian selvittämistä varten ja käytä tarvittaessa uutta tuloskansiota.
Valmiin videon uudelleenkäyttö ei myöskään ole todistus Piperin koko
riippuvuusympäristön välimuistin hyväksynnästä: dependency_closure_verified pysyy
false-tilassa.

## Mistä tulokset ja kuitit löytyvät

Jokaisella ryhmällä on oma antamasi --output-kansio. Sen sisältä löytyvät:

- render-inputs.json: lähteiden, profiilin, ääniresurssien ja ohjelmien tunnisteet.
- ep01, ep02 ja muut kyseisen ryhmän jaksokansiot: sequence.mp4, audio.wav,
  pipeline-report.json, render-receipt.json ja mahdolliset review-alkuiset PNG:t.
- compilation: ryhmän sequence.mp4, yhdistetty audio.wav ja render-receipt.json.
- logs: vientiohjelman, FFmpegin ja FFproben stdout- ja stderr-lokit.
- group-report-alkuiset JSON-tiedostot: erillinen raportti kustakin yrityksestä.

Jaksokuitti tallentaa todellisten MP4-, WAV- ja putkiraporttitiedostojen SHA-256-
tiivisteet sekä FFproben havaitsemat koodekit, kuvakoon, ruutumäärän ja keston.
Ääniraita on mono PCM16 / 24 kHz ennen AAC-vientipakkausta; videossa tarkistetaan
H.264-kuva ja AAC-ääni. status=completed tarkoittaa tämän työkalun tarkistaman
ryhmätuotannon valmistumista, ei julkaisun tai taiteellisen laadun hyväksyntää.

Ääni ja animaatio käyttävät samaa 24 kHz:n näytekelloa. Kun kuvanopeus on 24 fps,
yksi kuvaruutu vastaa täsmälleen tuhatta ääninäytettä. Koosteen WAV yhdistetään
alkuperäisestä PCM:stä ja vain jakson viimeinen vajaa kuvaruutu täydennetään
hiljaisuudella, alle tuhat näytettä jaksoa kohden. Puhetta ei leikata, eikä AAC-pakkauksen lisäämää lopputäytettä anneta kertyä
jaksojen väliin.

## Mitä pitää vielä hyväksyä

Manifestin ja kuittien publishable/publication_approved-arvot pysyvät false-tilassa.
Lähdepaketti ja onnistunut renderöinti eivät yksin sulje seuraavia töitä:

- Äänen, mallien, moottorin ja muun aineiston käyttö- ja julkaisuoikeuksien arvio.
- Hahmojen, kuvituksen, ilmeiden, kameratyön ja koomisen ajoituksen taiteellinen katselmointi.
- Huulisynkan vertailu käsin merkittyyn oikeaan ääneen ja kuunteluarvio.
- Oikean, hyväksytyn Kilsat-sovelluskaappauksen lisääminen. Nykyinen ESIMERKKI-
  merkinnällä varustettu näkymä on kuvitettu rekvisiitta, ei oikea kuvakaappaus.
- Fyysisen Macin koko työnkulun, laitteiden ja sovitun 300 sekunnin tavoitteen
  mittaaminen. Linux- tai CI-ajo ei sulje tätä laitehyväksyntää.

Tee julkaisu erillisen hyväksynnän ja tarkistuksen jälkeen omassa julkaisukanavassasi.
Tämä työkalu tuottaa paikallisen, tarkistettavan toimituksen ja säilyttää avoimet
hyväksynnät näkyvissä.
