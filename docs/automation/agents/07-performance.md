# 07 — Viiden minuutin suorituskyky ja työn ajoitus

Tila: **suunnitelma, ei toteutettu tai mitattu palvelutasolupaus**. Tämä osa
määrittelee, millä ehdoin 300 sekunnin tavoite voidaan myöhemmin hyväksyä. Se ei
väitä Pixar-/Disney-laatua tai GPU-kiihdytystä.

## Rajattu ensimmäinen profiili

Syöte on 30–60 sekunnin käsikirjoitus, enintään kaksi valmista hahmoa, kolme
tarinakohtausta ja 16 lopullista otosta, reaktiot mukaan lukien. Lopputavoite on
1920×1080, 24 fps, paikallinen MP4 ja todellinen suomalainen puhe. Tarinakohtaus ja
otoslaajennuksen tekninen `Scene` ovat eri asioita. Monimutkaisuusrajat sisältävät
myös käytetyt maskit, kuvat, tekstuurit, efektit ja äänen; hahmojen lukumäärä ei
yksin rajaa renderöintityötä.

300 sekunnin ajanotto alkaa käyttäjän **Luo**-komennon hyväksymisestä ja päättyy
vasta, kun lopullinen media ja muokattava projektitiedosto on kirjoitettu ja
tarkistettu. Esivalmistelut eivät saa hävitä raportista: käyttöliittymä näyttää
erikseen sovelluksen käynnistyksen, mallin latauksen ja aineistojen käyttöönoton.
Lämmin profiili edellyttää, että hyväksytyt paikalliset mallit ja aineistot ovat
jo saatavilla. Kylmän ajon kokonaisaika mitataan lisäksi; sille ei anneta samaa
300 sekunnin lupausta ennen omaa hyväksyntää. Verkkolatauksia ei käynnistetä
huomaamatta.

## Todettu nykytila ja ensimmäinen tekninen este

Tarkastettu lähde: `7bb5e21331a672b15be5b2227c93ccc8171e8fa7`.

| Nykyinen rajapinta | Todettu toiminta | Mitä vielä tarvitaan |
| --- | --- | --- |
| `motion_jobs::ordered_map`, `Config` | 1–4 kiinteää työntekijää, rajatut erät, tulokset indeksijärjestyksessä, virhe- ja peruutuspolkujen liittyminen | Koko tuotantoputken määräaika ja vaihekohtainen telemetria |
| `motion_jobs::MemoryBudget`, `process_budget()` | Prosessin yhteinen 1 GiB arvioitu varauskirjanpito, RAII-vapautus; ei OS:n muistikatto | Puuttuvien puhe-PCM-, egui- ja GPU-varauksien kattavuus sekä sivuprosessin RSS-mittaus |
| `motion_jobs::Graph`; `sequence_worker::run_stages` | Riippuvuudet, sykli- ja viitevirheet, rajattu ajo; scene → mix → peaks; tuotannossa `PREPARE_WORKERS = 1` | Tämän dokumentin koko DAG ja tarkistettu turvallinen puhepalvelun rinnakkaisuus |
| `motion-export/src/lib.rs`, `frame_jobs` | Enintään neljä ruututyöntekijää; 16 tavua/pikseli + 1 MiB per PNG-työntekijä; 320 MiB paikallinen arvio | Lopullisen profiilin työ- ja tallennusbudjetti |
| Sama vienti, rasteripolku | Enintään miljardi pikseliä/ruutusarja, 256 MiB tuotostavuja ja 16 miljoonaa pikseliä/ruutu | Rajattu suoratoistava vienti tai muu erikseen tarkistettu budjetointi; ei rajojen poistamista |
| `docs/jobs-performance.md` | Yhden M1-koneen 720p-vektorikohtauksen 48 ruudun vientimittauksia ja synteettistä hiljaista PCM:ää | Täysi 30–60 s jakso oikealla puheella, aineistoilla ja MP4-enkoodauksella |

**1080p-tavoite ei sovi sellaisenaan nykyiseen koko ruutusarjan rasterirajaan.**
30 s × 24 × 1920 × 1080 = 1 492 992 000 pikseliä; 60 sekunnissa määrä on
2 985 984 000. Molemmat ylittävät miljardin rajan. Myös 720p/60 s ylittää sen.
Tämä on ennen nopeusmittausta ratkaistava vientirajapinnan tehtävä. Sovellus ei
saa kiertää sitä pienentämällä kuvaa käyttäjältä piilossa. 256 MiB:n PNG-tavuraja
riippuu kuvien pakkaantuvuudesta, joten sitä ei voi ennustaa pelkästä kestosta.

Erikseen valittava nykykoodin referenssiprofiili voi olla **960×540, 24 fps**:
60 s:n pikselityö on 746 496 000 ja sopii tämän yksittäisen rajan sisään. Muut
rajat ja laatuportit tarkistetaan silti. Se nimetään käyttöliittymässä ja
raportissa eri profiiliksi; sitä ei saa esittää FullHD-tuloksena tai käyttää
1080p-tilauksen hiljaisena korvikkeena.

## Aikabudjetti ja DAG

Seuraavat ovat **suunnittelukattoja**, eivät mitattuja kestoja. Sarjalliset katot
ovat yhteensä 300 s. Rinnakkaisuudesta saatavaa mahdollista säästöä ei ole laskettu
lupaukseen kahteen kertaan. Puhebudjetti kattaa molemmat hahmot yhteensä.

| Vaihe | Katto | Edellytys ja tulos |
| --- | --- | --- |
| Tuonti | 5 s | Syöte → validoitu `ScriptIntent v1` |
| Ohjaussuunnittelu | 10 s | Intent → kohtaus-, otos- ja näyttelyaikomukset |
| Aineistojen tarkistus | 10 s | Viitteet → tarkistetut paikalliset hashit ja käyttöoikeusmerkinnät |
| Puhe | 60 s | Lukitut repliikit + hyväksytty ääniprofiili → todellinen PCM ja aikaleimat |
| Kohdistus | 20 s | PCM → ajoitukset, alkuperä ja mitattu tai arvioitu tila |
| Näyttely ja lopullinen ajoitus | 20 s | Äänen kesto + suunnitelma → kompiloitu muokattava aikajana |
| Renderöinti | 145 s | Lukittu aikajana → kaikki oikeat lopulliset ruudut |
| Enkoodaus ja tarkistus | 20 s | Ruutujen/äänen MP4, metatiedot, loppuosan dekoodaus ja projektin tallennus |
| Vara | 10 s | Tilanteen raportointi ja rajattu lopullinen julkaisu tiedostoksi |
| **Yhteensä** | **300 s** | Viimeistään määräajalla onnistuminen tai yksiselitteinen virhe |

Riippuvuudet: tuonti → suunnittelu → aineistotarkistus. Sen jälkeen rungon
näyttelysuunnittelu ja puhe voidaan suunnitella riippumattomiksi töiksi; puhe →
kohdistus → lopullinen ajoitus → renderöinti → enkoodaus/tarkistus → julkaisu.
Lopullinen ajoitus riippuu sekä näyttelysuunnittelusta että todellisesta puheesta.
Äänetön esikatselusuunnitelma ei siis ole valmis jakso. TTS-työntekijöiden määrää
ei kasvateta nykyisestä yhdestä ilman muistia ja palvelun säieturvallisuutta
koskevaa näyttöä. Streaming-enkoodaus saa myöhemmin limittyä renderöintiin, mutta
raportti säilyttää molempien oman seinäajan ja koko ajon seinäajan.

60 s:n jakso on 1440 ruutua. 145 sekunnin renderöintikaton vaatimus on vähintään
1440 / 145 = **9,94 lopullista ruutua sekunnissa**. Tämä on vaadittu läpimeno,
ei nykykoodista mitattu tulos. Yhden helpon ruudun nopeudesta ei johdeta koko
jakson, puheen tai MP4:n valmistumisaikaa.

## Sääntökatalogi

Pienempi numero tarkoittaa tässä osassa korkeampaa prioriteettia. Nämä ovat
ehdotetun orkestroijan sääntöjä, eivät uusia `RuleEffect`-komentoja.

| ID | Prioriteetti | Ehto → päätös | Virhe-/ristiriitapolku |
| --- | --- | --- | --- |
| PERF-01 | 0 | Profiili ja kaikki kustannusarviot mahtuvat rajoihin → hyväksy työ | `ProfileUnsupported` ennen sivuvaikutuksia; nimeä ylittävä raja |
| PERF-02 | 1 | Aineistot/ääni/malli eivät ole paikallisesti käyttövalmiita → lämmin profiili ei kelpaa | Näytä valmistelu tai kylmä profiili erikseen; ei hiljaista latausta |
| PERF-03 | 2 | Jobin riippuvuudet valmiit ja varaus saatu → käynnistä rajatussa poolissa | Puuttuva viite/sykli hylätään ennen ajoa; varauspula `BudgetExhausted` |
| PERF-04 | 3 | Tarkka sisältöavain ja ehjä tulos → käytä välimuistia | Hash-/versiovirhe on miss; osittaista tiedostoa ei käytetä |
| PERF-05 | 0 | Monotonisen kellon deadline saavutettu → peruuta ja raportoi | `DeadlineExceeded`, ei valmista mediaa eikä onnistumismerkkiä |
| PERF-06 | 0 | Puhe, ruutu tai tarkistus epäonnistuu → pysäytä riippuvat työt | Todellinen alkuperäinen virhe; ei hiljaista ääntä, pienempää fps:ää tai laatua |
| PERF-07 | 4 | Kaikki sisällölliset ja tekniset portit hyväksytty → atominen lopullinen julkaisu | Keskeneräinen tiedosto jää vain tunnistettuun työtilaan, ei lopulliseen nimeen |
| PERF-08 | 1 | Käyttöliittymä reagoi peruutukseen → välitä token heti | Mittaa erikseen tokenin vastaanotto ja viimeisen työn liittyminen |

Tasatilanteessa järjestys on vakaa job-ID:n mukaan. Satunnaissiemen vaikuttaa
vain suunnittelun sallittuihin valintoihin, ei työn valmistumisjärjestykseen tai
välimuistin osuman aiheuttamaan sisältöön. Nykyinen `Graph` ei keskeytä jo käynnissä
olevaa tehtävää. Siksi todellinen kova deadline edellyttää rajattuja
keskeytyskohtia sekä hallittua sivuprosessin pysäytystä TTS:lle/FFmpegille.
Pelkkä kellon tarkistus ennen jobia ei toteuta viiden minuutin maksimia.

## Sisältöavain ja resurssit

Avain muodostetaan kanonisesta suunnitelmasta, lähteen hashista, kääntäjän ja
sääntökatalogin versiosta, siemenestä, asset-hasheista, renderöintiprofiilista ja
taustamoottorin tarkasta identiteetistä. Puhe sisältää lisäksi tekstin, kielen,
äänen, malliversion ja akustiset parametrit; kohdistus sisältää PCM-hashin,
tunnistinympäristön, tekstin ja asetukset. Käyttäjän muokkaus mitätöi vain siitä
riippuvat tulokset. Keskeneräiset ja vahingoittuneet tulokset eivät koskaan ole
osumia. Nykyisten TTS-/alignment-välimuistien sääntöjä käytetään; niitä ei korvata
rinnakkaisella epäyhteensopivalla toteutuksella.

1080p-ruudun nykyinen työntekijäarvio on 34 226 176 tavua; neljä työntekijää
varaa noin 130,6 MiB, minkä lisäksi ovat kuvat, ääni, projektit ja sivuprosessit.
Prosessin 1 GiB:n kirjanpitoa ja paikallisia rajoja noudatetaan samanaikaisesti.
Tämä ei ole fyysisen RAM-kulutuksen takuu. 8 GiB:n M1-koneella sivuprosessin ja
käyttöliittymän todellinen huippumuisti sekä swap mitataan erikseen. Työt eivät
odota varausta käyttöliittymäsäikeessä; muistipula antaa korjattavan virheen.

## Mittaus ja hyväksyntä

Säilytä yksi schema-versioitu raportti per ajo: build-commit, release/debug,
macOS-versio, laitemalli, siru, RAM, käytetty CPU/GPU-polku, lähtösyötteen ja kaikkien
aineistojen hashit, ääniprofiili/kieli, lämmin/kylmä tila, vaiheiden alut/loput
monotonisesta kellosta, koko seinäaika, jobien omat ajat, ruutumäärä, resoluutio,
fps, välimuistin osumat, renderer-throughput, ulostulon hash, tiedostokoko,
huippu-RSS/sivuprosessi-RSS/swap ja validointitulokset. Valitsematta jäävät
mittarit ovat `null`, eivät nollia. Käyttöliittymän p95-päivitysaika ja
peruutus→liittyminen kirjataan; niitä ei nimetä todelliseksi input-to-photon-viiveeksi.

Esimerkkitilan `status: not_run`, `elapsed_s: null`, `deadline_met: null` täytyy
pysyä sellaisena, kunnes kyseinen tuotantoajo on oikeasti suoritettu. Raportti ei
saa muuttaa arvioita havainnoiksi.

Hyväksyntä tehdään vasta vientibudjettien ja puhepolun valmistuttua: yksi
lämmittely ja vähintään 20 lämmintä toistoa sekä 5 kylmää toistoa kummallekin
30 s:n ja 60 s:n edustavalle testille samalla nimetyllä Macilla. Julkaise kaikki
raaka-ajat, ei vain paras. Lämpimän profiilin jokaisen ajon koko seinäajan pitää
olla ≤300 s ja kaikkien laatuporttien pitää läpäistä; katkotut tai epäonnistuneet
ajot lasketaan epäonnistumisiksi. Tämä antaa rajattua näyttöä kyseisestä
profiilista, ei todista kaikkien tulevien käsikirjoitusten absoluuttista maksimia.

Välttämättömät testit: DAG-syklit ja deterministinen virhe; budjetin vapautuminen
virheessä/peruutuksessa; 1080p-työmäärän ennakkohylkäys nykyrajoilla; deadline
kesken ruudun/äänen/enkoodauksen; muuttunut malli/asset invalidioi osuman;
serial/parallel seek-tuotokset yhtenevät; puuttuva suomalainen ääni estää
valmistumisen; dekoodattu äänen loppu ja viimeinen ruutu säilyvät; UI:n kuormitustesti
fyysisellä Macilla. Ajastetut fake-clock-testit todentavat logiikan, eivät
suorituskykyä tai ammattitason visuaalista laatua.

Seuraava toteutusjärjestys: rajattu streaming-vienti ja kustannuspreflight →
kaikkien vaiheiden monotoninen deadline/peruutus → sisältöavaimet ja raportti →
nimetty fyysinen Mac-mittaus → vasta sitten hyväksytty profiili käyttöliittymään.
