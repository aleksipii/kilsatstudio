# Agentti 09 — laatu ja hyväksynnän näyttö

Tila: **suunnitelma**. Alla olevat rajat ovat ehdotettuja hyväksyntäehtoja,
eivät tämän työn mittaustuloksia. Ammattimainen alkuperäinen tyylitelty animaatio
on laatutavoite; Pixar-/Disney-pariteettia ei luvata eikä päätellä testien määrästä.
Koonti ja toteutusjärjestys: [automaation suunnitelma](../README.md).

## Nykyinen näyttö ja esteet

Tarkastettu lähde on `7bb5e21331a672b15be5b2227c93ccc8171e8fa7`.
`motion-script` validoi rakenteen ja viitteet; `expand_shots` ja `expand_rules`
tuottavat nykyisen kohtausputken aineistoa. Tämä ei arvioi näyttelyn uskottavuutta.
`motion-export` käyttää yhteistä näytekelloa ja tarkistaa manifestin puhe-/viseme-
välit. `MAX_BYTES` on 256 MiB ja rasteriviennissä on miljardin kokonaispikselin
raja. 1080p × 24 fps × 60 s = 2 985 984 000 pikseliä: ehdotettu Full HD -profiili
ei nykyisellään läpäise tätä rajaa. Rajan poistaminen ei ole hyväksyttävä korjaus;
tarvitaan erikseen budjetoitu vientitapa ja mittaukset.

`motion_tts::SpeechRequest::validate` hyväksyy vain `en-us` ja `en-gb`.
Suomenkielinen loppupilotti tarvitsee todellisen hyväksytyn tuontipuheen tai
erikseen toteutetun ja varmennetun suomenkielisen palveluntarjoajan. Englannin
tekstiarvio ei ole suomalaisen huulisynkan laatunäyttö.

[Suorituskykyohjeen](../../jobs-performance.md) yhdessä M1-istunnossa mitatut
lyhyet 720p-fixturet ovat olemassa olevaa rajattua näyttöä. Ne eivät todista
60 sekunnin pilottia, tämän automaation kokonaiskestoa tai fyysisen käyttöliittymän
hyväksyntää. [Master-progress](../../master-progress.md) erottaa jo testit,
fyysisen Macin havainnot ja lisenssihyväksynnän; uutta automaatiota ei merkitä
hyväksytyksi vanhojen osatulosten perusteella.

## Tarkistettava sopimus

Syöte on muuttumaton `ScriptIntent v1`, `DirectionPlan v1`, asset-manifesti,
ääni-/ajoitusmanifesti sekä lopullinen toistettavissa oleva vienti. Enintään kaksi
hahmoa, kolme **tarinakohtausta**, 16 lopullista otosta reaktioineen ja 30–60 s.
Otoslaajennuksen tekniset `Scene`-arvot eivät ole tarinakohtausten lukumäärä.
Tulos `QualityReport v1` sitoo havainnot lähde-, sääntö-, asset-, ääni-, build- ja
vientitiedostojen tiivisteisiin sekä laite- ja profiilitunnisteeseen.

Jokainen havainto sisältää `rule_id`, `status`, `evidence_ref`, `source_span`,
`shot_id`, `start_sample`, `end_sample` ja korjattavan syyn soveltuvin osin.
Tilat ovat `not_tested`, `passed`, `failed`, `blocked` tai perusteltu
`not_applicable`. Puuttuva näyttö ei ole `passed`; vanhentunut tiiviste palauttaa
kyseisen tarkistuksen tilaan `not_tested`. Raportti erottaa koneellisen
kelpoisuuden, taiteellisen hyväksynnän, nopeustavoitteen ja julkaisuoikeuden.

## Säännöt ja virheet

| ID | Ehto ja ehdotettu mittaus | Epäonnistuminen |
| --- | --- | --- |
| `QUA-001` | Nykyinen parseri/validointi, lähdeviitteet ja asset-tiivisteet täsmäävät; kaikki sovelletut säännöt näkyvät päätösraportissa. | `InvalidInput` / `StaleEvidence`; raskas työ ei ala. |
| `QUA-002` | Kirjoitettu dialogi ja puhujat säilyvät täsmälleen; jokaisella repliikillä on oikea puheaineisto. Puuttuva repliikki tai kolmas ääni estää valmiin puheversion. | `MissingSpeech` / `UnsupportedLanguage`; äänetöntä versiota ei hyväksytä valmiiksi. |
| `QUA-003` | Lopullisen median ruutumäärä ja PTS-järjestys täsmäävät valittuun profiiliin. Dekoodattu kesto poikkeaa suunnitellusta enintään yhden videoruudun. A/V-loppueron tavoite on enintään yksi videoruutu; puhe ei ylitä viimeistä näytettä. | `MediaTiming`; ei leikata puhetta sopimaan. |
| `QUA-004` | Äänessä ei ole leikkautuneita näytteitä. Ehdotettu lopullisen miksauksen true-peak ≤ −1 dBTP; puheiden suhteellinen havaittu voimakkuus arvioidaan kuuntelussa. Puuttuva mittari merkitään testaamattomaksi. | `AudioQC`; ei pelkkää WAV-otsakkeen hyväksyntää. |
| `QUA-005` | Käsin merkityllä suomalaisella puheella ehdotettu sulkeumien/mediaanin ajoitusvirhe ≤ 80 ms ja p95 ≤ 150 ms, lisäksi katselu normaalinopeudella. Merkitään annotaattorit, erimielisyydet ja otoskoko. | `LipSyncQC`; `estimated`-provenienssi säilyy, kunnes erillinen arvio on tehty. |
| `QUA-006` | Jatkuvuus: hahmo-, prop-, puhelin-, katse- ja tilasuhteet eivät vaihdu leikkauksissa ilman kirjoitettua toimintoa. Automaattinen tunniste/akselitarkistus ja ihmisen katselu. | `ContinuityQC`; fyysinen kontakti ei ole todistettu vain luuston rajatarkistuksella. |
| `QUA-007` | Kilsat-kuva näkyy pilotissa yhtäjaksoisesti 5 s. Katselija pystyy lukemaan tarinan kannalta olennaisen tiedon normaalinopeudella valitussa lopullisessa koossa. Oikea käyttöliittymäkuva hyväksytään erikseen; havainne on merkitty havainneksi. | `ReadabilityQC` / `UnverifiedAppClaim`; ei keksittyä tuotetoimintoa tai oikeaksi esitettyä mockupia. |
| `QUA-008` | Otoskohtaiset rajaus-, katse-, jalka-/prop-kontakti-, poseeraus- ja suunmuotohavainnot tarkastetaan. Ehdotettu kriittisten virheiden sallittu määrä on nolla. | `ActingQC`; kauniin yksittäisruudun arvio ei hyväksy liikettä. |
| `QUA-009` | Toisto samalla syötteellä/seedaineistolla tuottaa saman normalisoidun suunnitelman. Satunnaisessa seek-järjestyksessä verrataan jaetun CPU-esikatselun ja viennin samoja ruutuja. Koodekin binäärinen MP4-identtisyys ei ole vaadittu kuvan identtisyyden sijaan. | `DeterminismQC` / `PreviewExportMismatch`; GPU ei kierrä keskeneräistä pariteettia. |
| `QUA-010` | Koko työ ja valmis tiedosto valmistuvat ≤ 300 s nimetyllä fyysisellä Macilla; julkaisulla on erillinen oikeus- ja aineistohyväksyntä. | `DeadlineExceeded` tai `ReleaseBlocked`; kumpikaan ei muutu rakenteellisella CI-tuloksella hyväksytyksi. |

Ristiriidan etusija on 1) kelpoisuus/oikeudet, 2) käyttäjän lukittu sisältö,
3) näkyvän ja kuuluvan median eheys, 4) taiteellinen laatutavoite, 5) nopeus.
Nopeus ei poista laatutarkistusta, muuta resoluutiota tai tee puheesta äänetöntä.
Yksi kriittinen `failed` estää `release_ready`-tilan. Käyttäjä voi tallentaa
selkeästi luonnokseksi merkityn tuloksen erikseen.

## Kultainen pilotti ja taiteellinen arvio

Uusi 50 sekunnin Aino–Rane-pilotti sisältää puheen, vastareaktion, prop-kontaktin,
5 sekunnin Kilsat-kuvan ja loppupunchlinen. Ensin tehdään yksi ohjattu,
taiteilijan hyväksymä tavoiteversio **alkuperäisillä** hahmoilla. Vertailuun ei
kopioida studiotalojen hahmoja, kuvia tai animaatioita. Sen jälkeen automaation
versio arvioidaan samalla näytöllä, äänenvoimakkuudella ja profiililla.

Kolme arvioijaa näkee satunnaistetussa järjestyksessä tavoite- ja automaatioversion
ilman tuotantotavan nimeä. Arvioitavat osa-alueet: tarinan ymmärrettävyys,
poseerausten luettavuus, ajoitus/painon tunne, kasvonäyttely ja kuulijan reaktiot,
kameran/kuvasommittelun jatkuvuus sekä äänen/suun/kuvan kokonaisuus. Asteikko 1–5:
1 estää ymmärtämisen, 3 toimiva luonnos, 4 julkaisukelpoinen viimeistelty,
5 erityisen vahva. Ehdotus: jokaisen alueen mediaani vähintään 4, yhdenkään
arvioijan aluepiste ei alle 3 ja kriittisiä virheitä nolla. Tämä on rajattu
tuotantopaketin hyväksyntä, ei yleinen studio- tai elokuvatasoinen sertifikaatti.

Toistokoe käyttää vähintään kolmea ennalta valittua seediä, lisäksi samoilla
syötteillä kaksi uusinta-ajoa; hahmojen henkilöllisyys ja repliikit eivät muutu.
Pilotti, pidempi 60 s tapaus ja yksi kunkin rajan ylittävä kielteinen tapaus
säilytetään versionoituina. Vastakkaisiin suuntiin seekatut ruudut, leikkausten
rajat ja suun sulkeumat tarkastetaan erityisesti. Laatupalautteesta muuttuneet
säännöt käynnistävät koko pilottivertailun uudelleen.

## Viiden minuutin mittaus

Kello alkaa ensimmäisestä lopullisesta **Luo animaatio** -painalluksesta ja päättyy,
kun valmis mediatiedosto, laadun tulos ja avattava projektitiedosto ovat pysyvästi
kirjoitettu. Jonotus, käynnistys, valmis aineisto -tarkistus, puhe, kohdistus,
renderöinti, koodaus ja raportointi kuuluvat aikaan. Ihmisen käsikirjoitustarkistus
tehdään ennen tätä toimintoa. Jos työ vaatii uuden ihmispäätöksen, tulos on
`needs_review`, ei määräajassa valmistunut animaatio. Avoimen UI:n aiempi
lämmitysaika ja ladatut aineistot ilmoitetaan mittausprofiilissa.

Ehdotettu laitehyväksyntä: 20 peräkkäistä ajoa nimetyllä tavoite-Macilla,
kaikissa enintään 300 s ja kaikki laatugatet hyväksyttyinä. Raportissa säilytetään
jokainen aika sekä maksimi/p50/p95; myös epäonnistuneet ajot lasketaan.
Kylmäkäynnistys ja välimuistin tyhjä ajo raportoidaan erillisinä ilman yleistystä
lämmitetyn profiilin lupaukseksi. Muistipaine-, peruutus- ja UI-vastehavainnot
kerätään samalta fyysiseltä laitteelta. Välimuistiosuma ei korvaa syötteiden
identiteettitarkistusta. Tässä dokumentissa näitä ajoja ei ole tehty.

## Hyväksyntämatriisi tälle uudelle automaatiolle

| Portti | Tila nyt | Hyväksymiseen tarvittava näyttö |
| --- | --- | --- |
| Rajattu intentti ja sääntöjohdannaiset | `not_tested` | Uuden adapterin toteutus, raja-/ristiriita-/determinismitestit. |
| Nykyisen putken käyttäminen | `not_tested` | Tuotetun MotionScriptin varsinainen validointi, arviointi ja vienti; ei pelkkä suunnitelma. |
| Suomalainen puhe ja havaittu huulisynkka | `blocked` | Hyväksytty oikea FI-ääni, ajoitusnäyttö ja kuulijan/katsojan arvio. |
| Full HD -pikaprofiili | `blocked` | Budjetoitu nykyrajoihin sopiva vientitapa sekä todellinen media-/aika-/muistitesti. |
| Taiteellinen pilotti | `not_tested` | Säilytetty tavoite, kolme arvioijaa, pisteet ja korjaukset. |
| ≤ 300 s ja Mac-käyttöpolku | `not_tested` | Fyysisen Macin yllä määritelty ajosarja sekä seek/pause/ääni/UI. |
| Aineistot ja jakeluoikeus | `not_tested` | Tarkka aineistomanifesti, lisenssitekstit ja oikeuksien hyväksyntä. |

CI voi tarkistaa versionoidut sopimukset, virheluokat, lähdeviitteet,
deterministiset valinnat ja varsinaisen koneellisesti ajettavan median.
Suunnitelmadokumenttien CI tarkistaa vain suunnitelman yhtenäisyyden.
Kumpikaan ei merkitse yllä olevia uusia rivejä hyväksytyksi ilman niiden omaa
näyttöä. Fyysinen Mac, ihmisen havaintolaatu ja oikeudellinen hyväksyntä eivät
ole adapterittoman testin tai `not_applicable`-merkinnän korvattavissa.
