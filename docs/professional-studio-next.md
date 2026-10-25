# Kilsat Studio: seuraavat askeleet ammattimaiseen työskentelyyn

Arvioitu 2026-10-25. Lähtötilanne on integroitu PR #95–98 ja
[nykyinen toteutus- ja hyväksyntäseuranta](master-progress.md).
Tässä erotetaan olemassa oleva tekniikka, seuraavat käyttöliittymäparannukset ja
aidolla laitteella tai ihmisen arvioinnilla osoitettava laatu. Tavoite on oma,
itsenäinen satiirianimaation tuotantotyökalu, jolla jakson saa tehtyä alusta loppuun.

## Kolme tärkeintä asiaa juuri tähän käyttöön

1. **Hahmot, taustat ja esineet suoraan kohtaukseen.** Esikatselukuvallinen,
   haettava kirjasto, tiedostonvalitsimet ja selkeä uuden jakson työnkulku.
   Automaattitallennus ja palautus suojaavat keskeneräistä luovaa työtä.
2. **Animaation tekeminen kuvasta käsin.** Tartu käteen tai jalkaan, kiinnitä
   jalka lattiaan, tallenna asento ja säädä liike- ja kamerakäyriä. Käsirekvisiitan
   nykyinen koko projektin mittainen kiinnitys tarvitsee rinnalle ajastetut
   poimimis- ja irrottamiskontaktit sekä visuaalisen nivelmuokkauksen.
3. **Puhe, leikkaus ja ääni samassa näkymässä.** Dialogi, reaktiot, tauot,
   tehosteet ja musiikki omille raidoille. FI/EN-huulisynkan laatua parannetaan
   oikeasta puheesta mitatuilla virheillä ja kuuntelulla.

Näillä on tämän käyttäjän lyhyisiin satiirijaksoihin suurempi välitön hyöty kuin
laajan 3D- tai tehostevalikoiman rakentamisella. Alla olevat hyväksyntäehdot ovat
ehdotuksia seuraaville töille, eivät väitteitä jo läpäistyistä testeistä.

## Mitä on jo tehty

- **Oikeat videot ja puhe:** 31 suomenkielisen jakson ja seitsemän koosteen
  Full HD -viennit sekä kolme englanninkielistä Full HD -jaksoa on toteutettu.
  Suomen puhe käyttää paikallista Piper-profiilia tai omaa WAVia; englannin
  tuotetuissa jaksoissa on oikea Kokoro-puhe. Suomen puhesynteesi ei enää puutu.
- **Ohjaus ja kamera:** oikean äänen keston mukaan valmisteltu ohjaus,
  reaktiokuvat ja jatkuva, käyttäjän tarkastama pään kallistuksen tallennus
  ovat olemassa. Pään kallistus ei tarkoita kokovartalon tai ilmeiden kaappausta.
- **Editointi:** kohtauksia, hahmoja, tapahtumia, kameran avainruutuja ja
  kuvakerroksia voi muokata. Tapahtumia voi siirtää aikajanalla. Natiivi UI ja
  sovellukseen upotettu käyttöohje ovat suomeksi.
- **Tikkuhahmot ja käsirekvisiitta:** toinen sisältöversio yhdistää raajojen
  liitokset lantioon ja hartioihin sekä säilyttää pyöreät, ohuet tikkuviivat
  taivutuksissa. Yhdeksän käteen sopivaa esinettä sisältää puhelimen kolme
  näkymää. Kiinnitys seuraa valitun hahmon kättä kaikissa kohtauksissa.
  Viisi kuvakokopainiketta kirjoittaa olemassa olevia 2D-kameran avainruutuja.
  [Kuvallinen suunnitelma](stick-studio-art-direction.md) määrittää tarkistusehdot;
  niiden toteutuneet tulokset kirjataan erikseen seurantaan.
- **Uudelleenkäyttö:** alkuperäisrigin pose-/liikeklipit, JSON-kirjastovaihto,
  elävät klippi-instanssit ja lisättävä liike ovat toteutettuja. Omille rigeille
  on nivelraitojen talteenotto, kopiointi, aikavenytys ja manuaalinen nivelkartta.
- **Omien rigien kasvot:** valinnaiset suukartat, ilmevariantit, katse ja räpäytys
  ovat jo moottorissa. `evaluate_acting` valmistaa kasvot ja `apply_speech`
  valitsee suun samalla näytekellolla. Vanha rigi ilman näitä määrityksiä säilyttää
  perusosansa; tämä ei tarkoita, että kaikkien polygonirigien puhetuki puuttuisi.
- **Ääni ja säilytys:** puheen aaltomuoto, sampleihin sidottu suun käsieditointi,
  äänitehostecuet ja yhteinen miksaus ovat olemassa. Projektit tallennetaan
  atomisesti; yhteinen kumoa/toista-historia toimii istunnon aikana.
- **Vienti/GPU:** rajattu, suoratoiminen 1920×1080/24fps-vienti sekä laajennetut
  GPU-komennot on toteutettu. Komento-/pariteettitestit ja CI-videot eivät vielä
  osoita fyysisen Macin Metal-, ääni-, kamera- tai koko työnkulun hyväksyntää.

## Konkreettiset kehityskohteet

| Työ | Nykyinen pohja ja lähde | Seuraava parannus | Ehdotettu hyväksyntä |
| --- | --- | --- | --- |
| Sisältökirjasto | `main.rs::assets_ui` tuo, uudelleenlinkittää ja poistaa asset-viitteitä polkukentistä. | Kuvat, haku, luokat, suosikit, tiedostonvalitsin ja veto kohtaukseen. | Käyttäjä lisää hahmon, taustan ja esineen ilman polkujen tai YAMLin kirjoittamista; tallennus/avaus säilyttää ne. |
| Asennot ja liikkeet | `clip_library.rs`, `rig_clip_editor.rs`: kirjasto ja uudelleenkäyttö ovat jo olemassa. Oman rigin klippi kopioidaan; alkuperäisrigin live-ominaisuudet eivät siirry sille automaattisesti. | Kuvalliset asennot, peilaus, liikkeen kokeilu sekä selkeä rigiyhteensopivuus. | Kävely, osoitus, katse ja reaktio ovat valittavissa kirjastosta; ristiriitainen rigi antaa ymmärrettävän syyn. |
| Rigin muokkaus ja kontaktit | `custom_rig_editor.rs` tarjoaa YAML-/numerokentät, tekstuurit ja kahden nivelen ihotusvaikutukset. `hand_props.rs` kiinnittää esineen tavallisina osina käden niveleen kaikissa kohtauksissa. | Visuaaliset nivelet ja painot, IK/FK, jalan lukitus sekä esineen poimimisen/irrottamisen ajastus. | Esimerkkikävelyssä lukittu jalka ei luista yli yhtä maailmakoordinaattipikseliä; kahvikupin poimimishetki ja irrotus voidaan säätää ilman koko projektin rigin vaihtamista. |
| Puhuvan oman hahmon laatu | `motion-rig::evaluate_acting` ja `motion-scene::apply_speech` tukevat määriteltyjä suu-/ilmeosia jo nyt. `scripts/stick_characters.py` määrittää uuden kuuden hahmon kaikki 13 visemiä, silmät, kulmakarvat ja neljä tunneilmettä. | Suun ja ilmeiden helppo visuaalinen tarkastus sekä oikealla FI/EN-puheella tehty laatumittaus; puhumaton vanha rigi erotetaan valmiiksi määritellystä hahmosta. | Oikea valmisteltu FI/EN-repliikki liikuttaa uuden hahmon suuta esikatselussa ja viennissä; annotoitu ajoitus ja kuuntelu osoittavat laadun, eivät pelkät suukartan olemassaolo tai äänetön demo. |
| Liikekäyrät ja avainruudut | `keyframe_editor.rs`: x/y/kierto/mittakaava sekä Linear/Smoothstep/Hold. | Vedettävät avainruudut, monivalinta, arvo-/nopeuskäyrät ja viereisten asentojen haamukuvat. | Reaktion ennakointi, pysähdys ja jälkiliike voi säätää kuvallisesti; kumoa ja tallennus säilyttävät täsmälliset ajat. |
| Kuvakoot ja leikkaus | Kameraraidat, kohtausjärjestys, ristihäivytykset ja tapahtuman alun siirto ovat olemassa; `timeline_edit.rs` säilyttää Move/Gesture-keston. `camera_framing.rs` rajaa yleiskuvan, kahden hahmon kuvan, puolikuvan, lähikuvan tai käsiesineen lähikuvan olemassa olevalla 2D-kameralla. | Kuvakortit, leikkausrajat, klippien reunatrimmaus ja hallittu seuraavien kohtien siirto. | Käyttäjä rakentaa kuvakorteista puhujan lähikuvan ja kuulijan reaktion sekä säätää vitsin taukoa rikkomatta puhetta. |
| Ääniraidat | `audio-cues.md`: dialogi ja cuet miksataan samalla 24 kHz:n kellolla; cueilla on aloitus, gain, trim ja kesto. | Omat dialogi/tehoste/musiikkiraidat, cue-aaltomuodot, solo/mute, panorointi, häivytykset ja voimakkuusautomaatio. | Repliikin päällä oleva musiikki vaimenee säädetysti; tehoste on kohdistettavissa ruutuun; esikatselu ja vienti käyttävät samaa miksausta. |
| Äänen tuonti ja viimeistely | Nykyiset tehosteet vaativat 24 kHz mono PCM16 WAVin; ääni summataan ilman limitteriä. | Hallittu stereo/48 kHz -tuonti ja muunnos, huipputaso- ja äänekkyysmittaus sekä vientiasetukset. | Tavallinen musiikkitiedosto voidaan valita, esikuunnella ja muuntaa taustatyönä; yliohjaus näkyy ja julkaisu täyttää valitun äänekkyystavoitteen. |
| FI/EN-huulisynkka | `manual-mouth-timing.md`: arvioiden tarkastus ja sampletarkka manuaalinen muokkaus; `lipsync-timing-evaluation.md`: rakenteen/energian arviointi. | Oikea puheeseen kohdistus, suun ajoituksen vedot aaltomuodossa ja versioitu käsin annotoitu vertailuaineisto. | Molemmille kielille raportoidaan onset/offset-virheiden mediaani ja p95 sekä ihmisen kuuntelutulos. Aineisto, menetelmä ja hyväksymisrajat päätetään ennen mittausta. |
| Palautuminen | `motion-project::save_atomic` suojaa edellistä tallennusta; `unified-history.md` rajaa historian 64 tilaan/8 MiB:iin ja nykyiseen istuntoon. | Ajastettu palautuskopio, käynnistyksen palautusvalinta ja viimeisimmät projektit. | Pakotetun sulkemisen jälkeen palautuu viimeisin luvattu palautuspiste; alkuperäinen tiedosto säilyy ja käyttäjä valitsee palautuksen. |
| Pitkä tuotanto ja renderöintijono | `jobs-performance.md`, `fullhd-performance.md`: rajatut taustatyöt, peruutus, välimuistit ja atominen vienti; Full HD -profiili on enintään 60 s/vientityö. | Näkyvä jakso-/kuvajono, muuttuneiden osien uudelleenvienti, esikatseluversiot sekä keskeytyksestä jatkaminen tarkistetuista tuloksista. | Viiden minuutin jakso valmistuu hallituista osista; peruutus/uudelleenkäynnistys ei käytä vanhaa ääntä tai viallista välimuistia. Koko putken aika ja muistihuippu mitataan fyysisellä Macilla. |
| Viimeistely ja jakelu | H.264/AAC sekä PNG/SVG-vaihtopolut ja nykyinen GPU-renderöinti ovat olemassa. | Tarpeen mukaan läpinäkyvä master, värinhallinta ja selkeät toimitusprofiilit; aidon laitteen GPU-todisteet. | Samat referenssiruudut, tekstitykset, ääni ja värit tarkistetaan sovitulla Macilla ja viennissä; tuettu laite/backend kirjataan. |

## Pienet toteutusvaiheet

**Vaihe A — ensimmäinen oma satiirikohtaus.** Tässä lisäyksessä sisältöpaketti
sisältää kuusi tikkuhahmoa, neljä taustaa ja 20 esinettä sekä sovelluksen
aloituspainikkeen. Toinen sisältöversio korjaa nivelten aukot, parantaa taustojen
syvyyttä ja lisää yhdeksän käsirekvisiittavaihtoehtoa sekä kuvakokopainikkeet.
Hahmojen suu-/ilmekartat ja silmäohjaus on jo määritelty; malliprojekti on
äänetön. Sen 30 sekuntia havainnollistavat käsiesineitä ja vaihtuvia rajauksia.
Lisäyksen PR, tarkistustulokset ja yhdistämistila kirjataan erikseen
`docs/master-progress.md`-seurantaan. Se ei vielä toteuta koko sisältökirjastoa,
IK:ta tai studiota valmiiksi eikä osoita uuden hahmokaartin puheen laatua.
Jatkossa lisää kuvaselain, tiedostonvalitsimet ja palautus; testaa uuden
projektin luominen, kahden hahmon sijoitus, tallennus ja uudelleenavaus.

**Vaihe B — näyttely ja koominen rytmi.** Täydennä yhteensopivien rigien
asentokirjaston kuvallista käyttöä, ajastettuja käsikontakteja ja jalan lukitusta. Käytä uuden
hahmokaartin olemassa olevia suukarttoja oikean puheen kokeessa. Lisää sitten
käyrät ja kuvakortit. Hyväksyntänä yksi 30–60 sekunnin kohtaus, jossa on
ennakointi, vitsi, kuulijan reaktio, esinekontakti ja kolme tarkoituksellista
kuvakokoa. Kuvallinen ja rytmillinen arvio tehdään katsomalla ja kuuntelemalla.

**Vaihe C — toistettava julkaisu.** Yhdistä äänen raidat ja puheen ajoitus samaan
leikkaustyöhön. Lisää pitkä vientijono ja tuotannon tarkistusraportti. Testaa
FI- ja EN-kohtaus sekä viiden minuutin jakso aidolla Macilla kylmästä aloituksesta
valmiin videon itsenäiseen tarkistukseen. Aiempi CI/VM-aika ei korvaa tätä mittausta.

## Mitä ArtCraft-linkistä kannattaa ottaa vertailuun

Viralliset tuotesivut tarkistettu 2026-10-25. Nämä ovat valmistajan kuvaamia
toimintoja, eivät tässä kokeiltuja tai laadultaan varmennettuja sovelluksia.
[Sovellusluettelo](https://getartcraft.com/apps) merkitsee nämä neljä kehityksessä
oleviksi; muissa sovelluksissa on myös early alpha -merkintöjä.

| Vertailu | Hyödyllinen tavoite Kilsatille |
| --- | --- |
| [VectorCraft](https://getartcraft.com/apps/vectorcraft) | Hahmojen ja esineiden suora kuvallinen muokkaus, valinta ja kohdistus. |
| [EffectCraft](https://getartcraft.com/apps/effectcraft) | Liikekäyrät, sommittelutasot ja käyttäjälle näkyvä renderöintijono. |
| [SoundCraft](https://getartcraft.com/apps/soundcraft) | Moniraitainen editointi, miksaus ja voimakkuusautomaatio. |
| [FilmCraft](https://getartcraft.com/apps/filmcraft) | Leikkaus- ja trimmaustyökalut, kuvan tarkistus sekä äänekkyyden mittaus. |

Vertailu koskee yleisiä työnkulkuja. [AGENTS.md](../AGENTS.md) kieltää Storytoldin
lähdekoodin kopioinnin, riippuvuudet, integraatiot ja yhteensopivuusadapterit.
Kilsatin työkalut toteutetaan itsenäisesti nykyisen yhteisen arviointi-/vientiputken päälle.

## Julkaisuhyväksyntä pidetään erillään ominaisuuksista

- Fyysinen Mac: asennus ja käynnistys, äänen ulostulo, kamera, pitkä käyttö,
  tallennuksen palautus ja todellinen GPU/Metal-suorituskyky.
- Ihmisen arvio: FI/EN-äänen ymmärrettävyys ja lausunta, annotoitu huulisynkka,
  siluetit, näyttely, kuvakoot ja komedian ajoitus. Pelkkä testimäärä ei mittaa näitä.
- Oikeudet: täsmälliset malli-, ääni-, riippuvuus-, hahmo-, musiikki- ja
  esinelähteet sekä jakeluilmoitukset. Kokoron hyväksyntä ei kata kaikkia oikeuksia.
- Jakelu: Developer ID -allekirjoitus, notarisointi ja lopullisen paketin
  käynnistyksen varmistus. Nykyinen ad hoc -allekirjoitettu esikatselu on eri asia.

Lähteinä ovat yllä nimetyt moduulit, [tuotantointegraatio](production-integration.md),
[projektimuoto](project-format.md), [klippikirjasto](animation-clip-library.md),
[omien rigien ominaisuudet](custom-rigid-characters.md) ja trackerin nykyiset osiot.
Trackerin vanhat päivätyt puuttumislistat eivät ohita myöhemmin integroitua toteutusta.
