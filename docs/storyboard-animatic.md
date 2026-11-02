# Kuvakäsikirjoitus, animatiikki ja otosten tilat

Esituotannon ja tuotannon seuranta Kilsat Studiossa:

1. **Otoslista** johdetaan käsikirjoituksesta automaattisesti.
2. **Kuvakäsikirjoitus** on tulostettava arkki, jossa on yksi paneeli otosta kohden.
3. **Animatiikki** on video, jossa jokainen otos näkyy paneelikuvana koko
   kestonsa ajan ja oikea ääni kuuluu alla.
4. **Otosten tilat** kertovat, missä vaiheessa kukin otos on.

Koodi: [`motion-script/src/shot_list.rs`](../crates/motion-script/src/shot_list.rs)
(otoslista), [`motion-project/src/production.rs`](../crates/motion-project/src/production.rs)
(tilat), [`motion-export/src/storyboard.rs`](../crates/motion-export/src/storyboard.rs)
(arkki), [`motion-export/src/streaming.rs`](../crates/motion-export/src/streaming.rs)
(animatiikki) ja [`studio-desktop/src/shot_board.rs`](../apps/studio-desktop/src/shot_board.rs)
(työpöytänäkymä).

## Otoslista

`motion_script::shot_list(&script)` on puhdas funktio: sama käsikirjoitus antaa
aina samat otokset. Uusi otos alkaa:

- jokaisen kohtauksen alussa,
- kovassa leikkauksessa: kaksi kamera-avainta alle puolen ruudun välein.
  Näin tarinakääntäjä ja otoskirjasto kirjoittavat leikkauksen.
- kamerakulman vaihtuessa (`front`, `reverse`, `high`, `low`), koska kulma ei koskaan liu'u.

Kamera-ajo kahden kaukaisemman avaimen välillä kuuluu samaan otokseen.
Otokset numeroidaan kohtauksen sisällä 010, 020, 030 ja niin edelleen.
Otoksen tunnus on `<kohtaus>/<numero>`, esimerkiksi `kahvila_aamu/020`.

Jokaisella otoksella on:

- alku ja loppu
- kuvakoko zoomista: laaja alle 1,25, keskikuva alle 1,95, muuten lähikuva
- kamerakulma
- repliikit ja toiminnot (eleet, tunteet, kävelyt, asennot, kääntymiset ja esineet)
- **paneelihetki**: ensimmäisen eleen huippu, muuten 0,4 s ensimmäisen repliikin
  alusta, muuten ensimmäisen kävelyn puoliväli, muuten otoksen puoliväli.
  Hetki osuu aina ruutuun otoksen sisällä.

## Otosten tilat

Tilat tallennetaan `.kilsat`-projektiin kenttään `production`, jota ei
kirjoiteta, kun se on tyhjä. Vanhat projektit pysyvät siksi tavu tavulta samoina.
Tilat eivät ole käsikirjoitusta eivätkä kuulu kumoa-historiaan, kuten
tekstitysvalintakaan.

| Tila | Merkitys |
| --- | --- |
| `storyboard` – Kuvakäsikirjoitus | Oletus: otoksella ei ole merkintää. |
| `animatic` – Animatiikki | Ajoitus on hyväksytty animatiikissa. |
| `layout` – Layout | Kamera, lavaste ja sijoittelu ovat paikallaan. |
| `blocking` – Blokkaus | Pääasennot ja ajoitus. |
| `spline` – Välivaiheet | Liikkeet ovat sujuvia. |
| `polish` – Viimeistely | Hienosäätö. |
| `done` – Valmis | Hyväksytty. |
| `retake` – Korjattava | Palautettu korjattavaksi. |

Merkintä tallentaa otoksen alkuhetken. Jos kohtaukseen myöhemmin lisätään
leikkaus, numerot siirtyvät, ja merkintä, jonka otos alkaa nyt eri kohdasta
(yli 0,05 s), näytetään varoituksella "Otos on siirtynyt". Merkinnät, joiden
otosta ei enää ole, listataan, ja ne voi poistaa. Muistiinpano on enintään 2000 merkkiä.

Työpöydällä **Vie → Otokset ja tilat** näyttää kaikki otokset, tilojen
yhteismäärät ja suodattimen. Jokaisella otoksella on tilan valinta ja
muistiinpano, joka tallentuu, kun kenttä menettää kohdistuksen. Tallenna
projekti säilyttääksesi tilat.

## Kuvakäsikirjoitus

**Vie → Koko projekti / kohtauskooste → Kuvakäsikirjoitus (HTML)**, kun projekti on
valmisteltu. Uuteen kansioon tulee:

- `storyboard.html`: suomenkielinen arkki, kolme paneelia rivissä, tulostettavissa
  vaakasuuntaiselle A4:lle selaimesta
- `panels/*.png`: paneelit 640 pikselin pitkällä sivulla
- `storyboard.json`: samat tiedot koneluettavina

Paneelit piirretään samalla renderöijällä kuin esikatselu ja vienti (kuvitus,
kamera, tekstitys ja valinnainen tekstitysvalinta). Teksti suojataan HTML:ää
varten. Vienti ei korvaa olemassa olevaa kansiota, ja keskeytetty tai
epäonnistunut vienti poistaa oman kansionsa.

## Animatiikki

**Animatiikki (MP4)** samassa paikassa, kun FFmpegin polku on annettu. Jokainen
otos kootaan kerran paneelihetkestään ja näytetään koko otoksen ajan. Ääni
(puhe, tehosteet, musiikki) kulkee oikeassa ajassa samalla näytekellolla kuin
täysi vienti. Kokoa ja kestoa rajoitetaan:

- koko on puolikas: 1920×1080 → 960×540, jos molemmat sivut puolittuvat
  parillisiksi, muuten alkuperäinen koko
- enintään 10 minuuttia (14 400 ruutua 24 fps:llä)
- viennin määräaika 15 minuuttia ja enintään 1 GiB tulosta
- julkaisu kuten virtaavassa viennissä: `audio.wav`, `sequence.mp4` ja
  `sequence.json` uuteen kansioon kerralla, ja `sequence.json` listaa paneelit
  (`mode: animatic-held-panels`)

Komentoriviltä (ilman puhesynteesiä, siis äänettömänä):

```sh
cargo run --release -p motion-export --example storyboard -- \
  examples/stick-studio/stick-studio.kilsat /uusi/kansio /opt/homebrew/bin/ffmpeg
```

## Tarkistukset ja rajoitukset

- Testit kattavat leikkausten, kulmien ja kohtausten tunnistuksen,
  paneelihetket, tilojen tallennuksen, siirtyneet ja orvot merkinnät,
  arkin sisällön ja HTML-suojauksen sekä paneelien oikeat kuvat (väritesti).
  Lisäksi ne kattavat keskeytyksen siivouksen ja animatiikin paneelien
  ajoituksen. Oikealla FFmpegillä ajettu testi osoittaa, että pidetty paneeli
  ei vaihdu kesken otoksen.
- Tikku-ukkostudion aloitusprojektista on viety arkki ja 30 sekunnin
  animatiikki. Arkki on katsottu kuvana.
- Työpöytäsovelluksen näkymää ei ole vielä käytetty käsin, eikä sitä ole
  testattu fyysisellä Macilla.
- Kuvakäsikirjoitus syntyy valmiista animaatiosta. Piirrettyjä luonnospaneeleja
  ei voi vielä tuoda ennen animointia.
- Leikkaukseksi tunnistetaan vain kovat leikkaukset. Kohtausten väliset ristihäivytykset
  aloittavat uuden otoksen kohtauksen alussa.
