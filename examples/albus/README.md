# Albus

Albus on kitaraa soittava harmaa kissa. Hahmo perustuu käyttäjän omaan
piirrokseen. Tunnusmerkit ovat sateenkaaren värinen otsanauha, pyöreät lasit,
keltaiset silmät ja pystysuorat pupillit, väljä valkoinen collegepaita,
valkoiset cargohousut ja tennarit, sateenkaariranneke sekä valkoinen
sähkökitara hihnassa. Hahmo on tehty paperinukkeriggiksi samaan tyyliin kuin
`examples/cutout-cast`. Mitään ei ladata verkosta.

## Avaa sovelluksessa

1. Avaa `albus.kilsat` projektin avauskentästä.
2. Kohtaus **liikkeet** (25 s) näyttää Albuksen liikkeet pienellä lavalla.
3. Kohtaus **ilmeet** (17 s) käy läpi ilmeet, katseen ja käännökset.
4. `characters/albus.kilsat` sisältää pelkän hahmon.
   **Model sheet ja hahmotyyli** -paneeli piirtää siitä kääntökuvan, ilmeet,
   suun muodot ja siluettitarkistuksen.

## Ilmeet

| Tunne | Mitä tapahtuu |
| --- | --- |
| `neutral` | Silmät auki, pystypupillit, pieni kissan "w"-suu. |
| `happy` | Silmät sirrillään (^ ^), avoin hymy. |
| `sad` | Yläluomet roikkuvat ulkoreunoilta, korvat painuvat alas ja suupielet laskevat. |
| `surprised` | Pupillit laajenevat pyöreiksi, korvat nousevat pystyyn ja suu menee O:lle. |
| `angry` | Luomet viistossa, kapeat viirupupillit, korvat litistyvät sivuille, kissa sähisee ja kulmahampaat näkyvät. |

Lisäksi Albus räpyttelee itsestään, katse seuraa kohtauksen `gaze_tracks`-raitaa,
ja suulla on kaikki 13 visemiä puhetta varten. Visemit on tehty, mutta tämä
äänetön esimerkki ei todista huulisynkan laatua.

## Liikkeet

Projektiin on tallennettu Albuksen omat nivelklipit (`rig_clips`), joita voi
käyttää muissakin kohtauksissa:

| Klippi | Kesto | Liike |
| --- | --- | --- |
| `soitto` | 4 s | Rämpyttää kitaraa, nyökyttää päätä tahdissa. |
| `soolo` | 3 s | Nopea soolo: nojaa taakse, tassu liukuu kaulaa pitkin. |
| `headbang` | 2,5 s | Pää heiluu jokaisella iskulla, kakkosilla. |
| `jalka_vahvistimelle` | 3,5 s | Piirroksen asento: oikea jalka vahvistimen päällä, keinuu tahdissa. |
| `tassun_nuolaisu` | 2,5 s | Nostaa tassun suulle ja nuolee sitä kuin kissa. |
| `venyttely` | 3 s | Iso kissan venytys kädet ylhäällä ja pieni ravistus. |

Riggissä on samat 15 nivelen nimeä kuin muilla hahmoilla, joten yhteiset eleet
(`wave`, `jump`, `bow`, `shrug`, `nod`, `thumbs_up` jne.) toimivat myös.
Liikekohtauksessa näkyvät `jump`, `wave` ja `bow`.

Hahmo animoidaan kakkosilla (`timing: {on: 2, turn: snap}`) kuten muutkin
paperinukkehahmot.

## Tekijyys ja tiedostot

Tiedostot tuotetaan skriptillä `scripts/generate_albus.py`. Aja se uudelleen
muutosten jälkeen. Projektin MotionScript (`source`) kirjoitetaan luettavaksi
YAMLiksi: osiot omille riveilleen kommentteineen ja lyhyet avainkehykset
yhdelle riville, joten sitä voi lukea ja muokata sovelluksen skriptieditorissa.
Skripti käyttää paperinukkehahmojen yhteisiä apufunktioita
tiedostosta `scripts/generate_cutout_cast.py`. Testi
`crates/motion-export/tests/albus.rs` tarkistaa riggin, model sheetin, ilmeet ja
molemmat kohtaukset. Ympäristömuuttuja `ALBUS_OUT=<kansio>` tallentaa model
sheetin, ilmeet ja liikkeet PNG-kuviksi.

Kuvat on tarkistettu silmämääräisesti testin PNG-tulosteista. Sovellusta ei ole
ajettu tämän paketin kanssa.
