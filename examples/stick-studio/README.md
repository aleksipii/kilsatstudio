# Tikku-ukkostudio 3

Kuusi omaperäistä nivelhahmoa, 13 taustaa (niistä yhdeksän kaupunkia),
42 rekvisiittaesinettä ja 30 sekunnin muokattava malliprojekti. Kaikki projektit toimivat ilman ulkoisia
kuva-, puhe- tai mallilatauksia. Esittely on tarkoituksella **äänetön**.
Toinen sisältöversio korjaa nivelten näkyvät aukot, lisää käteen kiinnitettävän
rekvisiitan ja vaihtelee mallikohtausten kuvakokoja. Kolmas sisältöversio
lisää kaupungin, jossa hahmot voivat seikkailla (ks. **Kaupunki** alla).

## Aloita sovelluksessa

1. Avaa uuden version Koeta Studio ja paina **Tikku-ukkostudio**.
   Vaihtoehtoisesti avaa `stick-studio.kilsat` projektin avauskentästä.
2. Valitse vasemmalta kohtaus ja sen hahmo. Paina **Toista**.
3. Valitse näyttämön **Hahmot**-tila ja vedä hahmoa hiirellä
   uuteen paikkaan. Liike tallentuu aikajanan valittuun hetkeen. **Kumoa**
   palauttaa muutoksen. Pään ja muiden nivelten avainruutuja voi muokata
   niveltyökaluilla. Tämä paketti ei lisää visuaalista IK-editoria.
4. Pidä näyttämö **Hahmot**-tilassa ja valitse oikealta **Muokkaa → Esine käteen**.
   Valitse esine ja vapaa käsi sekä paina **Kiinnitä käteen**. **Säädä otetta**
   avaa siirron, kierron ja koon säädöt; **Vaihda esine / ote** tallentaa muutoksen.
   **Poista käsiesine** poistaa tämän työkalun lisäämän esineen. Muutos koskee
   valittua hahmoa **kaikissa kohtauksissa**. **Kumoa** palauttaa sen.
5. Valitse **Kamera ja kuvakoko** -kohdasta **Yleiskuva**, **Hahmot samaan kuvaan**,
   **Puolikuva**, **Lähikuva** tai **Kädessä oleva esine**.
   Rajaus tallentuu toistopään kohdalle;
   hahmon tai kädessä olevan esineen pitää olla valitussa kohtauksessa.
   Esineen lähikuva tarvitsee tällä työkalulla kiinnitetyn esineen.
6. Valitse näyttämön **Kuvitus**-tila ja oikealta **Muokkaa**, jotta voit
   muokata taustojen ja esineiden geometrisia tasoja.
   `backgrounds/`, `props/` ja `characters/` sisältävät myös erilliset projektit.
7. Tallenna omalla nimellä ennen pidempää työskentelyä. Malliprojektin avaaminen
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
Pyöristetyt, tasapaksut raajat liittyvät lantion ja hartioiden poikkiviivoihin.
Selkäranka taipuu vartalon nivelen kohdalta ja kaula ulottuu pään sisään.
Viisi uudelleenkäytettävää nivelklippiä ovat `odotus`, `vilkutus`, `osoitus`,
`reaktio` ja `askel`. Ne löytyvät projektin nivelklippityökalusta.
Askelklippi on liikeluonnos; siinä ei ole lattiaan lukittuvaa IK-jalkakontaktia.

Hahmojen asusteilla on omat piirroksensa puolisivulta, sivulta ja takaa
(`views`, ks. [custom-rigid-characters.md](../../docs/custom-rigid-characters.md)).
Silmälasit ja suojalasit pysyvät silmien kohdalla hahmon kääntyessä ja
katoavat takaa katsottuna. Solmio, liivin vetoketju ja lipan reuna piilotetaan
takaa, ja Sisun hiukset ja nuttura piirretään pään taakse. Piirrokset tehdään
skriptillä `scripts/stick_characters.py`.

## Ympäristöt ja rekvisiitta

**Taustat:** toimisto, bussipysäkki, hautausmaa ja kahvila sekä yhdeksän
kaupunkilavastetta (alla).

**20 alkuperäistä erillistä esinettä:** viikate, kahvikuppi, toimistotuoli, työpöytä,
kannettava tietokone, puhelin näyttöpuolelta, ajopäiväkirja, kello,
liikennemerkki, paketti, työkaluavain, sateenvarjo, auto, vuoronumerolaite,
puhelin takaa, puhelin vinosta, kirja, avain, tabletti ja kynä.

**Yhdeksän käteen kiinnitettävää vaihtoehtoa:** puhelimen kolme näkymää,
kahvikuppi, kirja, avain, tabletti, kynä ja työkaluavain. Ne määritellään
`hand-props.json`-luettelossa. Esine ja sen edessä näkyvät sormet seuraavat
kättä saman nivelarvioinnin kautta kuin hahmo. Tämä on koko projektin mittainen
kiinnitys; esineen ajastettu poimiminen tai irrottaminen ei vielä kuulu työkaluun.

Noutajan viikate, Päsmän lehtiö, Kiireen paketti ja Sisun kävelykeppi varaavat
niitä pitävän käden. Valitse hahmon toinen käsi. Työkalu säilyttää nämä pysyvät
tunnusesineet. Kaikkia erillisiä lavaste-esineitä ei voi kiinnittää käteen tämän
luettelon kautta.

Taustojen lattiat, kalusteiden ryhmittely ja etäiset maisemat luovat syvyyttä.
Pysäkin ja hautausmaan yhtenäinen siniharmaa tausta erottaa tummat raajat.
Malliprojektin vanha suuri harmaa soikio on poistettu.

Taustat ovat 1920 × 1080, yksittäiset hahmokuvat 768 × 1024 ja esinekuvat
512 × 512 pikseliä. Jaeltavassa kuvapaketissa on yhteensä 122 PNG/SVG-kuvaa.
Hahmot ja esineet ovat
läpinäkyviä PNG- ja SVG-kuvia. Taustat ovat peittäviä. SVG säilyttää
vektorigrafiikan, `.kilsat` lisäksi nivelrakenteet ja animaatiotiedot.
Kuvat ovat Koeta Studion omasta renderöijästä, eivät tyylikonseptin kuvituskuvia.
Mukana on 61 erillistä sisältöprojektia ja lisäksi kuuden kohtauksen malliprojekti.

## Kaupunki

Yhdeksän kaupunkilavastetta, jotka tarinatyökalu tarjoaa tunnisteillaan
(`set: keskusta` jne.). Jokainen on piirretty neljästä kamerakulmasta:
edestä ja vastakuvana käsin, ylhäältä ja alhaalta horisonttia siirtämällä.

| Tunniste | Lavaste | Vastakuva |
| --- | --- | --- |
| `keskusta` | Kauppakatu: leipomo, vaatekauppa, apteekki, liikennevalo | Kadun toinen puoli: bussi, taksi, pankki, elokuvateatteri |
| `asuinalue` | Vaahterakuja: kaksi omakotitaloa, aidat, auto pihassa | Kerrostalot ja leikkipuisto kadun takana |
| `tori` | Kauppatori: raatihuone kellotorneineen, torikojut | Tavaratalo, raitiovaunu ja patsas |
| `lahikauppa` | Kulmakaupan hyllyt ja juomakaappi | Kassa, liukuovet ja ostoskärryt |
| `huoltoasema` | Iltahuoltoasema: katos, mittarit, tankkaava auto | Autopesula ja maantie |
| `puisto` | Kaupunginpuisto: polku, ankkalampi, penkki | Leikkipuisto ja jäätelökoju |
| `katto` | Katto yöllä: vesisäiliö, antenni, valaistu siluetti | Porraskopin ovi, mainostaulu ja silta |
| `kuja` | Takakuja yöllä: takaovi, paloportaat, jäteastia, kissa | Kujan suu ja valaistu katu, jolla taksi |
| `metroasema` | Laituri ja odottava juna | Linjakartta, lippuautomaatit, liukuportaat |

**22 kaupunkiesinettä:** henkilöauto, taksi, poliisiauto, pakettiauto,
ambulanssi, kaupunkibussi, paloauto, skootteri, liikennevalo, paloposti,
pysäköintimittari, pankkiautomaatti, juoma-automaatti, katupuu, puistonpenkki,
suojatiemerkki, omakotitalo, kerrostalo, kulmakauppa, pilvenpiirtäjä,
polttoainemittari ja torikoju. Lavasteet käyttävät samoja piirroksia, joten
lavasteen auto on kirjaston auto skaalattuna ja peilattuna.

Kaupunki on mukana myös ankkurikirjastossa (`library/`): 22 esinettä
(`prop.vehicle.*`, `prop.street.*`, `prop.building.*` …) suomen- ja
englanninkielisin hakusanoin sekä yhdeksän taustaa (`background.*.city_street_v1`
jne., etukuva). Kaikki on alkuperäistä geometriaa (MIT, Koeta Studio), ilman
fontteja, kuvia tai latauksia; kylttien teksti on abstraktia viivaa.

Pitkä tarina: 12 kohtausta mahtuu taiteen 1024 tason rajaan, kun kohtaukset
kuvataan edestä. Jos jokainen kohtaus leikkaa myös vastakuvaan, raja voi
ylittyä; tarinatyökalu kertoo silloin, montako tasoa tarvittaisiin.

Piirrokset tehdään skriptillä `scripts/stick_city.py`; paketti generoidaan
`scripts/generate_stick_studio.py`:llä ja kirjaston esineet
`scripts/generate_city_library.py`:llä.

## Kuvakoot ja puhelimen näkymät

Kameran viisi kuvakokoa kattavat yleiskuvan, hahmot samassa kuvassa, puolikuvan,
lähikuvan ja kädessä olevan esineen lähikuvan. Rajaus käyttää arvioidun hahmon
tai esineen sijaintia. Voit muokata kameran avainruutuja myös käsin.
Kamera siirtää ja suurentaa 2D-kuvaa. Puhelimen etu-, taka- ja vinonäkymä ovat
erillisiä piirrettyjä esineitä; kameran siirto ei käännä hahmoa kolmiulotteisesti.

## Malliprojektin kohtaukset

| Aika | Kohtaus | Kuvassa |
| --- | --- | --- |
| 0–5 s | Vuoron alku, toimisto | Noutaja ja Veikko |
| 5–10 s | Pomo kutsuu, toimisto | Päsmä ja Kiire |
| 10–15 s | Pysäkkipaniikki, pysäkki | Veikko nostaa puhelimen korvalle |
| 15–20 s | Kahvitauko, kahvila | Sisu pitää kahvikuppia |
| 20–25 s | Keksintö, hautausmaa | Kipinä ja työkalu |
| 25–30 s | Mummo päättää, kahvila | Sisun lähikuva |

Lähikuvissa toinen hahmo on piilotettu kohtauksen näkyvyysasetuksella,
jotta sen kasvot tai tunnusesine eivät leikkaudu kuvan reunaan.

## Rajat ja seuraava kehitys

Tämä on muokattava aloituskirjasto, ei hyväksytty lopullinen sarjan ulkoasu.
Hahmodesign, liikkeiden luettavuus ja koominen ajoitus tarvitsevat taiteellisen
katselmoinnin. Ammattistudion seuraavat kehityskohteet ja hyväksymiskriteerit
ovat tiedostossa `professional-studio-next.md` (repositoriossa
`docs/professional-studio-next.md`). Fyysisen Macin käyttötestit sekä
tuotantoallekirjoitus ja notarisointi ovat erillisiä hyväksyntöjä.
Nivelten, otteen, taustojen ja rajausten tarkistuskriteerit on kuvattu
repositoriossa tiedostossa `docs/stick-studio-art-direction.md`.

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
