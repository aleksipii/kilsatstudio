# Tikku-ukkostudio

Kuusi omaperäistä nivelhahmoa, neljä taustaa, 14 rekvisiittaesinettä ja
30 sekunnin muokattava malliprojekti. Kaikki projektit toimivat ilman ulkoisia
kuva-, puhe- tai mallilatauksia. Esittely on tarkoituksella **äänetön**.

## Aloita sovelluksessa

1. Avaa uuden version Kilsat Studio ja paina **Tikku-ukkostudio**.
   Vaihtoehtoisesti avaa `stick-studio.kilsat` projektin avauskentästä.
2. Valitse vasemmalta kohtaus ja sen hahmo. Paina **Toista**.
3. **Muokkaa → Hahmot:** valitse hahmo esikatselusta ja vedä se hiirellä
   uuteen paikkaan. Liike tallentuu aikajanan valittuun hetkeen. **Kumoa**
   palauttaa muutoksen. Pään ja muiden nivelten avainruutuja voi muokata
   niveltyökaluilla. Tämä paketti ei lisää visuaalista IK-editoria.
4. **Muokkaa → Taide:** muokkaa taustojen ja esineiden geometrisia tasoja.
   `backgrounds/`, `props/` ja `characters/` sisältävät myös erilliset projektit.
5. Tallenna omalla nimellä ennen pidempää työskentelyä. Malliprojektin avaaminen
   on kumottava muutos; sovellus ei vielä sisällä automaattista palautustallennusta.

## Hahmot

| Hahmo | Luonne ja tunnusmerkki |
| --- | --- |
| **Noutaja** | Rauhallinen viikatemies. Kuiva huumori, musta huppu ja viikate. |
| **Veikko** | Tavallinen työmatkalainen. Sininen lippalakki ja olkalaukku. |
| **Päsmä** | Pikkupomo, joka tekee pienestä asiasta suuren. Punainen solmio ja lehtiö. |
| **Kipinä** | Keksijä, jonka ratkaisu tarvitsee uuden ratkaisun. Oranssi pipo ja suojalasit. |
| **Kiire** | Lähetti, joka ehtii kaikkialle paitsi pysähtymään. Limevihreä liivi ja paketti. |
| **Sisu** | Isoäiti, joka ei väistä edes Noutajaa. Silmälasit, violetti huivi ja kävelykeppi. |

Hahmoilla on sama 15 nivelen rakenne. Mukana ovat suun visemit, katse,
räpäytys ja ilmevariantit. Visemit ovat käytettävissä puheen animointiin;
äänetön malliprojekti ei todista puheen huulisynkan laatua.
Viisi uudelleenkäytettävää nivelklippiä ovat `odotus`, `vilkutus`, `osoitus`,
`reaktio` ja `askel`. Ne löytyvät projektin nivelklippityökalusta.
Askelklippi on liikeluonnos; siinä ei ole lattiaan lukittuvaa IK-jalkakontaktia.

## Ympäristöt ja rekvisiitta

**Taustat:** toimisto, bussipysäkki, hautausmaa ja kahvila.

**Esineet:** viikate, kahvikuppi, toimistotuoli, työpöytä, kannettava tietokone,
puhelin, ajopäiväkirja, kello, liikennemerkki, paketti, työkaluavain,
sateenvarjo, auto ja vuoronumerolaite.

Taustat ovat 1920 × 1080, yksittäiset hahmokuvat 768 × 1024 ja esinekuvat
512 × 512 pikseliä. Jaeltavassa kuvapaketissa hahmot ja esineet ovat
läpinäkyviä PNG- ja SVG-kuvia. Taustat ovat peittäviä. SVG säilyttää
vektorigrafiikan, `.kilsat` lisäksi nivelrakenteet ja animaatiotiedot.
Kuvat ovat Kilsat Studion omasta renderöijästä, eivät tyylikonseptin kuvituskuvia.

## Malliprojektin kohtaukset

| Aika | Kohtaus | Hahmot |
| --- | --- | --- |
| 0–5 s | Vuoron alku, toimisto | Noutaja ja Veikko |
| 5–10 s | Pomo kutsuu, toimisto | Päsmä ja Kiire |
| 10–15 s | Pysäkkipaniikki, pysäkki | Veikko ja Kiire |
| 15–20 s | Kahvitauko, kahvila | Noutaja ja Sisu |
| 20–25 s | Keksintö, hautausmaa | Kipinä ja Noutaja |
| 25–30 s | Mummo päättää, kahvila | Päsmä ja Sisu |

## Rajat ja seuraava kehitys

Tämä on muokattava aloituskirjasto, ei hyväksytty lopullinen sarjan ulkoasu.
Hahmodesign, liikkeiden luettavuus ja koominen ajoitus tarvitsevat taiteellisen
katselmoinnin. Ammattistudion seuraavat kehityskohteet ja hyväksymiskriteerit
ovat tiedostossa `professional-studio-next.md` (repositoriossa
`docs/professional-studio-next.md`). Fyysisen Macin käyttötestit sekä
tuotantoallekirjoitus ja notarisointi ovat erillisiä hyväksyntöjä.

Geometria on kirjoitettu tätä projektia varten itsenäisesti. Paketti ei sisällä
kolmansien osapuolten kuvia, fontteja, ääniä tai mallipainoja. Lähdekoodi ja
projektit noudattavat repositorion LICENSE-tiedostoa. Lopullinen hahmojen,
nimien ja muun tuotantosisällön oikeuksien tarkistus jää tuotantoa varten tehtäväksi.

## Toista vienti kehitysympäristössä

```sh
python3 scripts/generate_stick_studio.py
cargo build --release --locked -p motion-export --example stick_studio_assets --example production_export
python3 scripts/build_stick_studio_pack.py --asset-exporter "$PWD/target/release/examples/stick_studio_assets" --exporter "$PWD/target/release/examples/production_export" --ffmpeg /absolute/path/to/ffmpeg --ffprobe /absolute/path/to/ffprobe --output /absolute/path/to/new-output
```

Vienti tarkistaa lähdeprojektien SHA-256-tiivisteet, läpinäkyvyyden ja valmiin
MP4-videon kuvamäärän, koon, keston ja dekoodauksen. Nämä tekniset tarkistukset
eivät korvaa taiteellista tai laitetestausta.
