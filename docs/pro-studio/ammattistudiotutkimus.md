# Ammattitason animaatiostudiot — työkalut, tekniikat ja tyylit

Päivitetty 29.10.2026. Lähde: Claude-avustajan oma tietopohja (kesäkuuhun 2026 asti).
Verkkohakuja ei tehty. Tuoreimmat tuotenimet ja -versiot kannattaa tarkistaa ennen
kuin niihin viitataan julkaisuteksteissä.

Tämä dokumentti on taustatutkimus PRO-agenttisarjalle ([README](README.md)). Lopussa
on taulukko siitä, mikä havainto on annettu millekin agentille.

## 1. Ohjelmistot

### 2D-tuotanto (TV- ja suoratoistosarjat)

| Ohjelma | Käyttö |
| --- | --- |
| Toon Boom Harmony | TV-animaation standardi. Sillä tehdään sekä piirtämällä animointi että cut-out-rigit, joissa hahmo on koottu irrallisista paloista (luut, deformerit, piirrosten vaihto) |
| Toon Boom Storyboard Pro | Kuvakäsikirjoitukset ja animaatiot |
| TVPaint | Bittikarttapohjainen, perinteinen ruutu kerrallaan animointi. Suosittu Euroopassa ja elokuvissa (esim. Cartoon Saloon) |
| Moho | Luupohjainen cut-out-animointi pienemmissä studioissa |
| Adobe Animate, After Effects | Liikegrafiikka, selitysvideot ja kompositointi |
| Spine, Live2D, Rive | Reaaliaikaiset 2D-rigit peleihin ja sovelluksiin |
| Blender Grease Pencil | 2D-piirtäminen 3D-ympäristössä |
| OpenToonz, Krita | Avoimen lähdekoodin vaihtoehdot |

### 3D-tuotanto (elokuvat)

- **Maya**: animoinnin ja riggauksen standardi.
- **Houdini**: efektit ja simulaatiot.
- **Blender**: yleistyy nopeasti varsinkin pienissä studioissa.
- **ZBrush ja Substance**: veistäminen ja tekstuurit.
- **Renderöijät:** RenderMan, Arnold ja Karma.
- **Unreal Engine**: virtuaalituotanto ja reaaliaikaiset sarjat.

### Kompositointi, editointi, ääni ja tuotannonhallinta

- **Nuke** (kompositointi), **DaVinci Resolve**, **Avid** ja **Premiere** (editointi), **Pro Tools** (ääni).
- **Flow Production Tracking** (entinen ShotGrid), **ftrack** ja avoin **Kitsu** seuraavat otoksia, tehtäviä ja versioita.
- **SyncSketch**, **cineSync** ja **OpenRV** ovat katselmointityökaluja, joilla ruutuihin voi piirtää kommentteja.

## 2. Kirjastot ja avoimet standardit

Suurin osa on Academy Software Foundationin projekteja.

| Standardi | Mihin |
| --- | --- |
| OpenTimelineIO (OTIO) | Aikajanojen ja editointien siirto ohjelmasta toiseen |
| OpenEXR | Liukulukuiset kuvasekvenssit, joissa voi olla useita kerroksia |
| OpenColorIO + ACES | Hallittu väriputki |
| OpenUSD | Kohtausten kuvaus ja niiden kokoaminen kerroksista |
| Alembic | Valmiiksi lasketun geometrian siirto ohjelmasta toiseen |
| OpenImageIO, OpenVDB, MaterialX | Kuvien käsittely, volyymit (savu, pilvet), materiaalit |
| FFmpeg | Videon koodaus (ProRes, H.264/H.265) |
| Lottie / Rive | Kevyet vektorianimaatiot sovelluksiin ja verkkoon |

**Rust-puolella käyttökelpoisia:**
- `wgpu` (näytönohjaimen käyttö)
- `vello`, `lyon` ja `tiny-skia` (2D-vektorirenderöinti)
- `kurbo` (Bézier-geometria) ja `resvg` (SVG-kuvat)
- `symphonia`, `cpal` ja `rubato` (ääni)
- `ort` (ONNX Runtime, mallien ajo)
- `exr`-crate

Uusi riippuvuus vaatii aina A01:n riippuvuus- ja lisenssipäätöksen.

## 3. Animointitekniikat

- **Animaation 12 periaatetta** (Disney). Tärkeimmät hahmoanimaatiossa:
  - venytys ja litistys (squash & stretch)
  - ennakointi (anticipation)
  - jatkuva ja limittäinen liike (follow-through, overlapping action)
  - kaaret (arcs)
  - hidas lähtö ja pysähdys (ease in/out)
  - toissijainen liike (secondary action)
  - lavastus (staging)
- **Pose-to-pose:** ensin avainasennot (keys), sitten välimuodot (breakdowns) ja lopuksi
  välikuvat (inbetweens). Blokkausvaiheessa avaimet näytetään askeltavalla interpoloinnilla
  (stepped) ja vasta viimeistelyssä pehmeillä käyrillä (spline).
- **Animointi ykkösillä, kakkosilla tai kolmosilla:** sama kuva näytetään 1, 2 tai 3 ruudun
  ajan. Kakkosilla tehty liike näyttää "animaatiomaiselta" ja säästää työtä.
- **Pidot (holds) ja liikkuvat pidot (moving holds):** hahmo ei koskaan jähmety täysin
  paikalleen.
- **Smear-ruudut ja liike-epäterävyys** nopeissa liikkeissä.
- **Cut-out-riggaus:** hahmo rakentuu paloista luuhierarkian varaan. Riggeissä on IK/FK-vaihto,
  deformerit ja piirrosten vaihto, esimerkiksi suun ja käsien muodot vaihdetaan valmiista
  kirjastosta.
- **Huulisynkka:** ääni nauhoitetaan aina ennen animointia. Foneemit merkitään **X-sheetiin**
  (valotustaulukko, josta näkee ruutu ruudulta mitä näytetään). Suun asennoille käytetään
  vakiokarttaa, tavallisesti **Preston Blairin 8–10 suumuotoa**. Avuksi käytetään työkaluja
  kuten Rhubarb Lip Sync, Papagayo ja Montreal Forced Aligner.
- **Elokuvakieli:**
  - kuvakoot (laajakuva, puolikuva, lähikuva)
  - 180 asteen sääntö
  - leikkaus liikkeen kohdalla (cut on action)
  - toimintalinja (line of action)
  - hahmon siluetin luettavuus
- **Mocap ja tekoälyavusteiset työkalut:** liikekaappausta tehdään esimerkiksi Rokokolla,
  Vicon-järjestelmillä ja Move.ai:lla. Cascadeur auttaa fysiikkaa noudattavassa
  poseerauksessa, ja Autodesk Flow Studio (entinen Wonder Studio) on myös käytössä. Studiot
  käyttävät generatiivista tekoälyä varovasti tekijänoikeuskysymysten vuoksi.

## 4. Hahmotyylit

| Tyyli | Esimerkkejä | Tunnusmerkit |
| --- | --- | --- |
| Rubber hose | 1930-luvun Disney, Cuphead | Letkumaiset raajat ilman kyynärpäitä, ympyrämuodot |
| UPA / rajoitettu animaatio | Hanna-Barbera, UPA | Litteä, geometrinen, vähän ruutuja |
| Moderni litteä TV-tyyli | Adventure Time, Steven Universe | Paksut ääriviivat, yksinkertaiset muodot, helppo riggata |
| Anime | Ghibli, Trigger | Yksityiskohtaiset avaimet, rajoitettu välianimaatio, voimakkaat efektit |
| Geometrinen selitysvideotyyli | Kurzgesagt | Ääriviivaton, muodot ja värit kantavat ilmaisun |
| Tikku-ukko | Alan Becker, xkcd | Pelkkä liike ja siluetti, ei tekstuuria |
| Tyylitelty 3D | Spider-Verse, Arcane, Klaus | Maalauksellinen valaistus, animointi kakkosilla, 2D-efektit 3D:n päällä |

**Suunnitteluperiaatteet:**
- **Muotokieli:** ympyrä tarkoittaa ystävällistä, neliö vakaata ja kolmio vaarallista.
- **Siluetin pitää olla tunnistettava** myös täysin mustana.
- **Mittasuhteet** ilmaistaan päänkorkeuksina.
- **Model sheet**, **turnaround** (hahmo eri kulmista) ja **ilmeataulukko** pitävät hahmon
  samannäköisenä koko tuotannon ajan.
- **Rajattu väripaletti** hahmoa kohden.

Esikuvat ovat tyylisuuntia. Kilsat Studio ei kopioi yksittäisen teoksen hahmoja tai
grafiikkaa.

## 5. Esineet (props)

- **Esinekortit (prop sheets)** näyttävät mittakaavan suhteessa hahmoon, eri kulmat ja eri
  tilat (esimerkiksi puhelin auki ja kiinni).
- **Pääesineet ja taustaesineet erotetaan.** Pääesine on tarinan kannalta tärkeä ja
  yksityiskohtaisempi, taustaesine on yksinkertaistettu.
- **Esine kiinnitetään käden luuhun** (parenting) ja vaihdetaan tarvittaessa toiseen käteen.
  Usein mukana on erillinen tartuntakäsi.
- **Uudelleenkäytettävä esinekirjasto** noudattaa samoja nimeämis- ja tyylisääntöjä kuin
  hahmot.

## 6. Taustat

- **Layout** päättää kuvakoon, kameran ja hahmojen sijainnit. Vasta sen jälkeen
  taustamaalari tekee varsinaisen taustan.
- **Värikäsikirjoitus (color script):** koko elokuvan tai jakson tunnelma esitetään pieninä
  värikuvina.
- **Arvorakenne:** taustassa on vähemmän kontrastia ja kylläisyyttä kuin hahmoissa, jotta
  hahmo erottuu.
- **Monitasokamera (multiplane) ja parallaksi:** etu-, keski- ja taka-ala ovat eri tasoja,
  jotka liikkuvat eri nopeuksilla.
- **Ilmaperspektiivi:** kaukana olevat asiat ovat vaaleampia ja sinertävämpiä.
- **Toistuvat ja panoroitavat taustat** kävely- ja ajokohtauksiin. Samoja taustoja käytetään
  uudelleen eri kuvakulmista.

## 7. Tuotantoputki

Esituotanto: käsikirjoitus → kuvakäsikirjoitus → animatic (karkea kuvakäsikirjoitusvideo) →
hahmo- ja taustasuunnittelu → ääninauhoitus.

Tuotanto: layout → animointi → puhtaaksipiirto → väritys → kompositointi.

Jälkituotanto: editointi, äänisuunnittelu ja miksaus, värimäärittely. Koko ajan pidetään
yllä versionhallintaa ja nimeämiskäytäntöjä, ja jokaisesta otoksesta pidetään katselmointi.

## 8. Havainnoista agenttien tehtäviksi

| Havainto | Agentti | Tehtävätiedosto |
| --- | --- | --- |
| Askeltava/pehmeä interpolointi, animointi ykkösillä tai kakkosilla, blokkaus | P01 | [P01-ajoitus-blokkaus.md](P01-ajoitus-blokkaus.md) |
| Preston Blairin suukartta, X-sheet | P02 | [P02-suukartta-xsheet.md](P02-suukartta-xsheet.md) |
| 12 periaatetta komentoina, liikkuvat pidot | P03 | [P03-animaatioperiaatteet.md](P03-animaatioperiaatteet.md) |
| Kuvakoot, 180 asteen sääntö, leikkaus liikkeen kohdalla | P04 | [P04-elokuvakieli.md](P04-elokuvakieli.md) |
| Monitasotaustat, parallaksi, ilmaperspektiivi | P05 | [P05-taustat-parallaksi.md](P05-taustat-parallaksi.md) |
| Esinekortit, kiinnityspisteet, esineiden tilat | P06 | [P06-esineet-kiinnitys.md](P06-esineet-kiinnitys.md) |
| OTIO, PNG/EXR-sekvenssi, ProRes, väriavaruus | P07 | [P07-ammattilaisvienti.md](P07-ammattilaisvienti.md) |
| Model sheet, turnaround, siluetti, paletti, tyylit | P08 | [P08-model-sheet-tyylit.md](P08-model-sheet-tyylit.md) |
