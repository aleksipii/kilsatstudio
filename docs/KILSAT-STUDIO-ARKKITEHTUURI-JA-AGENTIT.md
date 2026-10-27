# KILSAT STUDIO: SÄÄNTÖPOHJAISEN 2D-ANIMAATIOSTUDION JÄRJESTELMÄARKKITEHTUURIRAPORTTI

> Lähde: Liitetty teksti(7).txt. Markdown-muunnos 26.10.2026.
> Alkuperäisen raportin sisältö on säilytetty alla. Luvut 6–8 ovat tätä tehtävää varten laadittu agentti- ja toteutussuunnitelma.
> Raportin nykytilaa, kirjastoja, tutkimusviittauksia tai suorituskykylupauksia ei ole tarkistettu lähdekoodista tai ulkoisista lähteistä. Kuvaus ei osoita ominaisuuksia toteutetuiksi.


## 1. JÄRJESTELMÄN TAVOITTEET JA AKATEEMISET TUTKIMUSPOHJAT

### 1.1 Deterministinen ja tekoälyvapaa (No AI) -arkkitehtuuriparadigma

Kilsat Studio -animaatiostudio on suunniteltu määrittelemään uudelleen kriittinen, suorituskykyinen ja täysin toistettava 2D-animaatiotuotanto. Nykyaikaisessa digitaalisessa graafisessa tuotannossa perustavanlaatuiseksi ongelmaksi on muodostunut järjestelmien riippuvuus stokastisista, koneoppimiseen pohjautuvista animaatiomalleista. Nämä mallit kärsivät arvaamattomasta satunnaisuudesta, suorituksenaikaisista viiveistä sekä roskienkeruun (Garbage Collection, GC) aiheuttamista ruudunpäivityksen nykimisistä. Kilsat Studion arkkitehtuurin kova ydin nojaa 100-prosenttisesti sääntöpohjaiseen ja täysin deterministiseen (No AI) paradigmaan. Tämä valinta takaa absoluuttisen toistettavuuden, millisekunnintarkan reaalikatasoisen suorituskyvyn sekä matemaattisesti todennettavan ilmeiden ja liikkeiden hallinnan ilman generatiivisten mallien epävarmuustekijöitä.

Perinteiset tekoälypohjaiset ja roskienkeruusta riippuvaiset animaatiojärjestelmät arvioivat animaatioruudut todennäköisyyspohjaisesti tai tilaan sitoutuneiden iteratiivisten simulaatioiden kautta. Tällöin saman syötteen ajaminen kahdesti voi johtaa erilaiseen visuaaliseen lopputulokseen, ja muistinhallinnan katkossa syntyvät GC-tauot rikkovat kuvaruudun päivitysjakson (16.6 ms / 8.3 ms). Kilsat Studio ratkaisee tämän soveltamalla puhtaasti koodipohjaista determinismiä, jossa järjestelmän tila missä tahansa ajanhetkessä on riippumaton aiemmista suorituskehyksistä.

| Arviointikriteeri | Tekoälypohjainen animaatio (ML / Generatiivinen) | Kilsat Studion sääntöpohjainen arkkitehtuuri |
| --- | --- | --- |
| Determinismi | Stokastinen / Arvaamaton; tuottaa vaihtelevia tuloksia samalla syötteellä. | Absoluuttinen; sama aika t ja säännöt tuottavat aina bitilleen identtisen ruudun. |
| Suorituskyky / Tick-aika | Vaihteleva ja raskas (GPU/NPU-päättely); suuri viive ja GC-stutter-riski. | Vakioitu ja monistettava; O(1)-sääntöindeksointi ja viiveetön, unallocating-koodipolku. |
| Reunaehdot ja vakaus | Epävakaa reuna-alueilla; "hallusinointi" ja visuaaliset artefaktit rajoissa. | Matemaattisesti rajattu; tiukat reunaehto-osiot estävät virhetilat. |
| Renderöinnin toistettavuus | Vaatii siemennystä (seed), ei takuuta alustariippumattomasta identtisyydestä. | 100 % alustariippumaton ja bitti-identtinen offline- ja live-renderöinnissä. |
| Resurssitarve | Korkea muistinkulutus ja massiivinen laskentakapasiteetin tarve. | Minimaalinen muistijalanjälki; suorituskykyinen jopa ilman ulkoisia muistivarauksia. |

Tämä ehdoton deterministinen paradigma kytkee Kilsat Studion suoraan digitaalisen muotoilun ja tietojenkäsittelyn klassisiin akateemisiin tutkimussuuntauksiin.

### 1.2 Akateemiset pilarit: CMU Rete, SFU MotionScript ja Brown BALSA

Kilsat Studion arkkitehtuuri nojaa vahvasti kolmeen tietojenkäsittelytieteen klassiseen läpimurtoon, joiden pohjalta moottorin logiikka, kieli ja visuaalinen esitystapa on rakennettu:

## 1. CMU Rete -algoritmi (Carnegie Mellon University): Monimutkaisten animaatiosääntöjen ja reaktiivisten tilamuutosten hallinta perustuu Rete-algoritmin johdannaiseen (RETE-UL). Algoritmi muodostaa alfa- ja beeta-muistisolmuista koostuvan verkon, joka mahdollistaa faktojen ja ehtojen tehokkaan kuvionsovittamisen. Tämä takaa sääntöjen O(1)-indeksoinnin ja reaktiivisen reitityksen sääntömäärän kasvaessa sadoista kymmeniin tuhansiin ilman lineaarista suoritusaikaista suodatusta.
## 2. SFU MotionScript DSL (Simon Fraser University): Animaation ajallinen ja rakenteellinen ilmaisu pohjautuu deklaratiiviseen domain-specific language (DSL) -kielioppiin. MotionScript mahdollistaa monimutkaisten animaatiosekvenssien ilmaisemisen yhdistelyprimitiivien kautta, kuten sarjallistaminen (seq!), rinnakkaistaminen (par!) ja vaiheistettu viive (stagger), erottaen visuaalisen laitteiston hallinnan animaation toiminnallisesta määritelmästä.
## 3. Brown BALSA (Brown University Algorithm Simulator and Animator): Järjestelmän graafisen tilan visualisointi ja tietorakenteiden dynaaminen esittäminen seuraavat BALSA-järjestelmän periaatteita. Sen mukaisesti jokainen tietorakenteen muutos tai tilakoneen siirtymä visualisoidaan tarkan, vaiheittaisen ja vaihekatkottoman tilasiirtymän kautta, mikä tekee monimutkaisista abstrakteista operaatioista välittömästi hahmotettavia.

Akateemisten teorioiden konkreettinen kääntäminen Kilsat Studion arkkitehtuurikomponenteiksi eritellään seuraavasti:

* RETE-UL Inference Engine: Vastaa reaktiivisesta tilanhallinnasta, jossa faktamuutokset laukaisevat välittömästi animaatiotilojen ja graafisten korvausten (Swaps) päivittymisen.
* Declarative Motion DSL: Tarjoaa ihmisluettavan YAML/GRL-syntaksin, joka muuntaa korkean tason käsikirjoituskomennot suoritettaviksi ratkaisemattomiksi rata-antureiksi (TrackSpecs).
* Deterministic Scene Graph & BALSA Inspector: Huolehtii tietorakenteiden absoluuttisesta visualisoinnista ja tilamodifiointien puhtaasta ajallisesta projektioinnista.

Nämä akateemiset algoritmit ja käsikirjoituskieliopit tarjoavat vankan pohjan reaaliaikaiselle matemaattiselle fysiikkasimulaatiolle, joka vastaa järjestelmän mekaanisesta luonnollisuudesta.

### 1.3 Fysiikan matemaattinen pohja: Analyyttiset vaimennetut jousiyhtälöt

Ihmisen näköaisti on äärimmäisen herkkä tunnistamaan mekaanisen liikkeen keinotekoisuuden. Luonnollinen liike fysikaalisessa maailmassa ei ala tai pysähdy ilman kiihtyvyyttä ja massan hitautta. Kilsat Studiossa hahmojen osien, orgaanisten elementtien ja käyttöliittymäkomponenttien elastinen liike perustuu differentiaaliyhtälöihin.

Perinteisesti animaatiojärjestelmissä käytetty numeerinen Eulerin integrointi (v_n = v_{n-1} + a \cdot \Delta t, x_n = x_{n-1} + v_n \cdot \Delta t) on vahvasti herkkä ruudunpäivitysnopeuden vaihteluille (\Delta t). Eulerin menetelmä kerryttää kumulatiivista numeerista virhettä, mikä johtaa näytteistystaajuuden pudotessa simuloinnin epävakauteen, ylilyönteihin tai ilmiön "räjähtämiseen" hallitsemattomaksi äärettömäksi heilahteluksi (infinite oscillation).

Kilsat Studio poistaa numeerisen epävakauden ratkaisemalla toisen kertaluvun lineaarisen homogeneettisen differentiaaliyhtälön analyyttisesti suljetussa muodossa:

$$
m \ddot{x} + c \dot{x} + k x = 0
$$

missä m on massa, c on vaimennuskerroin ja k on jousivakio (jäykkyys). Yhtälön fysikaalista käyttäytymistä ohjaavat reuna-arvot määritetään varaamalla suoraan vaimenematon kulmataajuus \omega_n ja dimensioton vaimennussuhde \zeta (zeta):

$$
\omega_n = \sqrt{\frac{k}{m}}, \quad \zeta = \frac{c}{2\sqrt{k m}}
$$

Vaimennussuhteen \zeta perusteella analyyttinen suljettu ratkaisu x(t) jakautuu kolmeen täsmälliseen matemaattiseen reuna-alueeseen:

## 1. Alivaimennettu (\zeta < 1): Luonnehtii systeemiä, jossa vaimennus on vähäistä. Systeemin ominaisarvot ovat kompleksikonjugaatteja, ja suljettu ratkaisu on: x(t) = e^{-\zeta \omega_n t} \left( A \cos(\omega_d t) + B \sin(\omega_d t) \right) missä \omega_d = \omega_n \sqrt{1 - \zeta^2} on vaimennettu kulmataajuus. Objekti heilahtelee maalikohteen yli eksponentiaalisesti vaimenevan siniaallon mukaisesti.
## 2. Kriittisesti vaimennettu (\zeta = 1): Ominaisyhtälöllä on kaksoisjuuri, ja analyyttinen ratkaisu saa muodon: x(t) = (A + B t) e^{-\omega_n t} Tämä tarjoaa nopeimman mahdollisen paluun tasapainotilaan ilman yhtäkään maalipisteen ylilyöntiä. Se on ihanteellinen vaste käyttöliittymäsiirtymille ja täsmällisille animaatiosiirtymille.
## 3. Ylivaimennettu (\zeta > 1): Systeemin vaimennus on suuri, ja yhtälöllä on kaksi erillistä reaalijuurta r_{1,2} = -\omega_n \left( \zeta \pm \sqrt{\zeta^2 - 1} \right), jolloin ratkaisu koostuu kahdesta eksponentiaalisesta vaimenemistermistä: x(t) = A e^{r_1 t} + B e^{r_2 t} Objekti hakeutuu maaliin hitaasti ja ilman heilahtelua.

Ihmisen eleiden ja interaktioiden vapautuksessa syntyvät visuaaliset nykäykset estetään säilyttämällä alkunopeus (from_velocity). Kun käyttäjä vapauttaa sovelluksessa raajan tai objektin kesken liikkeen, hetkellinen kiihtyvyys ja nopeus asetetaan differentiaaliyhtälön alkuarvoehdoiksi x(0) = x_0 ja \dot{x}(0) = v_0, jolloin liike jatkus saumattomasti ilman katkosta.

Nämä analyyttiset fysiikkayhtälöt sidotaan suoraan käsikirjoitusohjattujen animaatiotilojen ja aikajanojen täsmälliseen ajastukseen.

## 2. KÄSIKIRJOITUSOHJATTU ANIMAATIO- JA SÄÄNTÖMOOTTORI

### 2.1 Käsikirjoituskieli (MotionScript YAML / GRL DSL) ja synksyntaksi

Kilsat Studion käsikirjoitusjärjestelmä käyttää MotionScript YAML- ja GRL (Grule Rule Language) -synksyntaksia, joka muuntaa ihmisluettavan teksti-ilmaisun suoritettavaksi ja tyypitetyksi animaatio-oliosäilöksi. Rekisteröidyt entiteetit tunnistetaan selkeillä merkkijono-ID:illä, mikä mahdollistaa monimutkaisten lausekkeiden rakentamisen ilman osoitinlogiikkaa.

Seuraava esimerkkikoodi havainnollistaa kohtauksen entiteettien määritelmää, perustilaa sekä aikajanan verppejä:

```yaml
# MotionScript YAML / GRL DSL Scene Definition
scene:
  entities:
    - id: "node42"
      type: "circle"
      position: [300.0, 400.0]
      radius: 40.0
      style: "ink_stroke"
    - id: "code.line2"
      type: "code_line"
      text: "let mut engine = RuleEngine::new();"
      position: [100.0, 600.0]
      color: "FADED"
    - id: "cam"
      type: "camera"
      zoom: 1.0

timeline:
  - act:
      verb: "highlight"
      target: "code.line2"
      color: "ACCENT"
      duration: 0.3
  - act:
      verb: "fade_in"
      target: "node42"
      duration: 0.4
  - parallel:
      - act:
          verb: "move_to"
          target: "node42"
          position: [860.0, 400.0]
          duration: 0.5
          easing: "InOutCubic"
      - act:
          verb: "cam_to"
          target: "cam"
          position: [400.0, 300.0]
          duration: 0.5


```

Animaatioaktit ja -verbit kootaan kolmen matemaattisen koostamisprimitiivin avulla:

* seq![a, b, c]: Suorittaa animaatioklipit tiukasti peräkkäin (sarjallistaminen).
* par![a, b, c]: Suorittaa katsauksen klipit samanaikaisesti rinnakkain.
* stagger(delay, clips): Syöttää määritetyn aikaviiveen peräkkäisten klippien suoritukselle luoden porrastettuja ryhmä- ja verkkoreaktioita.

Käsikirjoituskielen verbit tuottavat ratkaisemattomia rata-antureita (TrackSpec), joiden pohjalta moottori laskee suhteellisten ja absoluuttisten siirtymien kumulatiivisen summan ennen interpolaatiota.

Tämä deklaratiivinen käsikirjoituskieli tarjoaa syötteen järjestelmän sääntömoottorin inferenssikoneistolle, joka reagoi tilamuutoksiin dynaamisesti.

### 2.2 RETE-UL-sääntömoottorin arkkitehtuuri ja alfa/beeta-muisti

Kilsat Studion reaktiivinen logiikka pohjautuu rust-rule-engine-crateen, joka toteuttaa RETE-UL-algoritmin. Algoritmi jakaa ehtolausekkeiden arvioinnin kahteen eri muistitasoon:

```text
[ Faktat (Facts) ]
       │
       ▼
 [ Alfa-solmut ] ── (Yksittäisten faktojan suodatus, esim. Type == Node)
       │
       ▼
 [ Beeta-solmut ] ── (Monimuuttujaliitokset, esim. Node.x > Boundary.x)
       │
       ▼
   [ Agenda ] ────── (Laukaistavien sääntöjen prioriteetti-jono)
       │
       ▼
 [ Säännön laukaisu ] ➔ [ Tilan päivitys (Working Memory) ]
```


* Alfa-muistisolmut: Suorittavat yksittäisten faktojen kenttäsuodatuksen O(1)-aikakompleksisuudella.
* Beeta-muistisolmut: Arvioivat useamman muuttujan välisiä liitosehtoja ja osittaisosumia säilyttäen aiemmat välitulokset muistissa.

Sääntömoottori tukee sekä suoraa ketjutusta (Forward Chaining / data-driven), jossa faktojen muuttuminen laukaisee reaktiivisesti uudet säännöt, että käänteistä ketjutusta (Backward Chaining / goal-driven), joka hyödyntää unifikaatiota ja DFS/BFS-hakua määrittäessään, voidaanko tietty animaatiotavoite todistaa nykyisestä tilasta käsin.

Järjestelmätason kytkentä MotionScript-käsikirjoituksen ja sääntömoottorin välillä toteutetaan reaktiivisesti: kun rust-rule-engine laukaisee säännön (RHS / then-osa), se tuottaa tai muokkaa lennossa broadsheet-moottorin TrackSpec-rakennetta ajanhetkellä t. Tämä mahdollistaa animaatioradan dynaamisen uudelleenreitityksen ja muuntamisen ilman koko aikajanan uudelleenlaskentaa.

Sääntömoottorin tuottamat tilamuutokset ja dynaamiset rata-anturit sidotaan välittömästi aikasidonnaiseen deterministiseen aikajanalaskentaan.

### 2.3 Deterministinen tilayhtälö: scene_at_time = f(script, rules, t) ja aikajanalaskenta

Järjestelmän absoluuttisena perusperiaatteena on, että animaatiokohtauksen jokainen ruutu on puhtaasti ilmoitetun ajan t funktionaalinen arvo. Tilaa ei arvioida suhteellisina edellisen ruudun muuntimina, vaan riippumattomana tilaleikkauksena:

$$
\text{scene\_at\_time} = f(\text{base\_scene}, \text{timeline}, \text{rules}, t)
$$

Tämä tilaton aikajanalaskenta tarjoaa merkittäviä etuja animaation muokkaus- ja toistokäyttäytymiselle. Aikajanalla siirtyminen (scrub), toiston pysäytys (pause), ruutukohtainen eteneminen (frame-step) sekä hyppy aikajanamerkkeihin tapahtuvat välittömästi ilman tarvetta simuloida tai välimuistittaa aiempia ruutuja 0 \dots t-1.

Käsikirjoituksen verbit tuottavat aluksi ratkaisemattomia rata-antureita (TrackSpec). Finalisointivaiheessa moottori käy läpi kaikki ratatiedot (entiteetti, ominaisuus) -parittain ja laskee kumulatiivisen asennon kaavalla:

$$
\text{Pos}_{\text{concrete}}(t) = \text{Pos}_{\text{base}} + \sum \Delta \text{Pos}_{\text{prior}} + \text{lurp}\left(\text{from}, \text{to}, \text{ease}(t_{\text{local}})\right)
$$

```text
[ TrackSpec: Rel(Vec2(100, 0)) ] 
              │
              ▼ (Lasketaan kaikkien aiempien ratojen summa)
[ Concrete Track: Track { from: Vec2(300, 400), to: Vec2(400, 400) } ]
```


Relatiiviset (Rel), absoluuttiset sekä palautustargetit (Revert) ratkaistaan yksiselitteisiksi avainruutuväliksi (Track { from, to }), jolloin toistohetkellä suoritetaan ainoastaan suoraa interpolaatiota.

Tämä puhdas funktionaalinen aikajana-arkkitehtuuri luo pohjan ilman tekoälymalleja toteutettaville prosesseille, jotka vastaavat Adobe Character Animatorin suorituskykyisiä animaatiotoimintoja.

## 3. ADOBE CHARACTER ANIMATOR -VASTAAVAT PROSESSIT ILMAN AI-MALLEJA

### 3.1 Klassinen algoritmipohjainen kasvo- ja kameraseuranta (OpenCV / ilman ML-malleja)

Kilsat Studio toteuttaa kasvojen ja kameran liikeseurannan ilman raskaita, epädeterministisiä koneoppimismalleja (No ML). Järjestelmä nojaa OpenCV-C-rajapinnan kautta ajettaviin deterministisiin, klassisiin kuvanprosessi- ja geometrisiin algoritmeihin.

Kamerasyötteen käsittelyssä käytetään värisuodatusta, konturien tunnistusta sekä Lucas-Kanade optista virtausta (Optical Flow) piirrepisteiden seuraamiseksi. Tunnistettujen geometristen piirteiden ja kiintopisteiden sijaintierot muunnetaan suoraan 2D-affiinimuunnosmatriiseiksi. Pään kallistus, kierto ja siirtymä ohjaavat suoraan hahmon hierarkkista muunnospuuta ilman neuroverkon suorittamaa arviointia, takaaten millisekuntitason vasteajan ja ehdottoman toistettavuuden samalla syötemateriaalilla.

Reaaliaikainen algoritmipohjainen seurantasyöte ohjaa suoraan hahmon dynaamisia fysikaalisia reaktioita ja lisäosien liikettä.

### 3.2 Fysikaaliset simulaatiot (Dangle, Tuuli, Painovoima ja Vaimennetut Jouset)

Hahmon orgaanisten lisäosien – kuten hiusten, vaatteiden ja korvakorujen – dynaaminen liike toteutetaan Dangle-fysiikkajärjestelmällä, joka hyödyntää luvun 1.3 analyyttisia vaimennettuja jousiyhtälöitä.

Systeemi yhdistää jousipotentraaliin ulkoiset voimavektorit, kuten säädettävän painovoimavektorin sekä tuulivektorin, jota ohjataan proseduraalisella aaltomuodolla ja turbulenssitekijöillä. Järjestelmä tarjoaa valmiit matemaattiset fysiikkaprofiilit eri materiaaleille:

| Profiili | Massa (m) | Jäykkyys (k) | Vaimennus (c) | Vaimennussuhde (\zeta) | Fysikaalinen käyttäytyminen |
| --- | --- | --- | --- | --- | --- |
| Snappy | 1.0\text{ kg} | 400.0\text{ N/m} | 40.0\text{ Ns/m} | \zeta = 1.00 | Terävä, välitön vaste ilman heilahtelua (kriittisesti vaimennettu). |
| Gentle | 1.5\text{ kg} | 100.0\text{ N/m} | 15.0\text{ Ns/m} | \zeta = 0.61 | Pehmeä, alivaimennettu ja hitaasti mukautuva orgaaninen liike. |
| Bouncy | 0.8\text{ kg} | 250.0\text{ N/m} | 8.0\text{ Ns/m} | \zeta = 0.28 | Herkästi heilahteleva alivaimennettu vaste joustaville lisäosille. |
| Stiff | 2.0\text{ kg} | 800.0\text{ N/m} | 80.0\text{ Ns/m} | \zeta = 1.00 | Raskas, lähes joustamaton materiaali terävillä pysähdyksillä. |
| Wobbly | 0.5\text{ kg} | 120.0\text{ N/m} | 3.0\text{ Ns/m} | \zeta = 0.19 | Voimakkaasti hyppelevä ja pitkään jatkuva elastinen liike. |

Fysikaalinen toissijainen liike yhdistyy saumattomasti hahmon visuaaliseen puhesynkronointiin ja foneettiseen animaatioon.

### 3.3 Automaattinen huuliosynkronointi (10 Viseemin Tila-automaatti & Ääniprosessointi)

Huuliosynkronointi perustuu puheen foneettiseen jakamiseen 10 distinctiin visuaaliseen suun muotoon eli viseemiin. Puhuttu kieli jaetaan tila-automaatissa seuraaviin viseemitiloihin:

| Viseemi-ID | Rive / Character -tila | Vastaavat Foneemit ja Kirjaimet |
| --- | --- | --- |
| 0 | Neutral | Tauko, välilyönti, lepotila |
| 1 | AEI | A, E, I |
| 2 | BMP | B, M, P |
| 3 | FV | F, V |
| 4 | CDGKNRSTXYZ | C, D, G, K, N, R, S, T, X, Y, Z |
| 5 | U | U |
| 6 | ChJSh | Ch, J, Sh, H |
| 7 | L | L |
| 8 | O | O |
| 9 | QW | Q, W |

Offline-esikäsittelyssä Python-alijärjestelmä suorittaa äänianalyysin (hyödyntäen Piper/Kokoro TTS -synteesiä tai WAV-ääniradan amplitudi- ja spekrianalyysia) tuottaen tarkan JSON-aikajanamatriisin:

```json
[
  { "chars": "H", "viseme": 6, "timeMs": 0 },
  { "chars": "e", "viseme": 1, "timeMs": 58 },
  { "chars": "ll", "viseme": 7, "timeMs": 112 },
  { "chars": "o", "viseme": 8, "timeMs": 180 }
]
```


Tämä matriisi syötetään RETE-sääntömoottorille, jossa sääntö phoneme.value = viseme vaihtaa hahmon suukerroksen tilan välittömästi ilman manuaalista avainruutujen asettelua.

Viseemikohtaiset tilasiirtymät kytkeytyvät osaksi laajempaa ilmeiden, asentojen ja eleiden Triggers & Swaps -vaihtojärjestelmää.

### 3.4 Triggers & Swaps -järjestelmä ja RETE-pohjaiset kytkökset

Hahmon ilmeiden, asentojen ja käsimuotojen vaihtaminen perustuu dynaamisiin laukaisimiin (Triggers) ja tekstiilikerrosten korvauksiin (Swaps). RETE-sääntömoottori suorittaa ehtotarkistukset O(1)-aikakompleksisuudella mahdollistaen saumattomat tilasiirtymät.

Alla oleva GRL-sääntöesimerkki havainnollistaa, kuinka syöte laukaisee käden tekstuurin vaihdon ja antaa animaatioklipille heilahtelukomennon:

```text
rule "Hand Wave Trigger" {
    when
        User.TriggerKey == "HandWave" && Character.ArmState != "Waving"
    then
        Character.SwapTexture("RightHand", "Hand_Open_Wave");
        Character.ArmState = "Waving";
        Animation.Play("RightArm_PingPong_Oscillation");
}
```


Triggers & Swaps -järjestelmän laukaisemat graafiset tilat siirtyvät suoraan hahmon sisäiseen luuranko- ja verkkodeformaatiorakennekerrokseen.

### 3.5 Luurankoriggaus, Käänteiskinematiikka (IK) ja Verkkodeformaatio (SkelForm)

Kilsat Studion luurankorunko ja mesh-deformaatio pohjautuvat avoimen lähdekoodin SkelForm-integraatioon (rusty_skelform). Järjestelmä tuorilintuaa PSD-kuvatiedostot suoraan hierarkkisiksi luurangoiksi pivot-pisteineen.

```text
[ Luuhierarkia (Bone Tree) ] ➔ [ Käänteiskinematiikka (IK Solver) ]
                                          │
                                          ▼
[ PSD-Tekstuuripinnat ] ➔➔➔➔➔ [ Verkkodeformaatio (Mesh Vertex Deform) ]
```


Käänteiskinematiikka (Inverse Kinematics, IK) mahdollistaa kahden tai useamman luun raajojen (esim. jalka tai käsi) luonnollisen taivuttamisen siirtämällä vain yhtä maolipistettä (IK Target). Verkkodeformaatiossa (Mesh Deformation / vertex-deform) 2D-tekstuuri sidotaan verteksiverkkoon, jolloin luun kierto taivuttaa ja venyttää tekstiilipintaa orgaanisesti. rusty_skelform-deformaatiomuistit ohjataan nollakopiona animato-gpu-laskentashadereille (tween.wgsl), mitkä laskevat verteksisiirtymät suoraan GPU:lla.

Luurankorimppauksen ja verkkodeformaation mahdollistamat rakenteet kytketään saumattomasti käyttäjän hiirellä ja kosketuksella ohjaamaan interaktiivisuuteen.

### 3.6 Hiirellä vedettävät raajat (Draggables) ja elefysiikka

Käyttäjän interaktiivinen syöte (hiiri tai kosketusnäyttö) sidotaan suoraan hahmon luurankorungon IK-maaleihin Draggables-arkkitehtuurin kautta.

Kun käyttäjä tarttuu raajaan ja liikuttaa sitä, järjestelmä laskee liikemomentin rullaavalla aikaikkunalla. Kun ote irrotetaan, järjestelmä ei pysäytä raajaa mekaanisesti, vaan liike-energia purkautuu liikekitkan vaimentumisena (Kinetic Friction Decay) yhdistettynä vaimennetun jousen palautusvoimaan. Tämä luo luonnollisen ja fysikaalisesti uskottavan elefysiikan.

Tämä interaktiivinen elefysiikka täydentää animaatiologistiikan ja siirtää huomion järjestelmän tekniseen suorituskykypinoon ja moduuliarkkitehtuuriin.

## 4. MONIKIELINEN TEKNOLOGIAPINO JA MODUULIARKKITEHTUURI

### 4.1 Rust-ydin: Suorituskyky, RETE-UL, wgpu ja SkelForm

Järjestelmän ydinsuorituskyvystä vastaa täysin ilman roskienkeruuta (GC) toteutettu Rust-ydinmoottori. Se takaa vakaat kuvaruutuajat ilman muistinhallinnan aiheuttamia nykimisiä. Ydin rakentuu seuraavista modulaarisista krateista:

* broadsheet: Aikajanalaskenta, tilaton scene_at_time-resoluutio ja suora avainruutujen deterministinen interpolointi.
* rust-rule-engine: RETE-UL-algoritmin toteutus, muisti-indeksit, forward/backward chaining sekä GRL-syntaksin jäsennin.
* animato: Analyyttinen differentiaaliyhtälöfysiikka, jousidynamiikka sekä 31 eri kasvukaavan loivennuskäyrää (perceptual easing curves).
* rusty_skelform: Luurankohierarkioiden laskenta, IK-ratkaisin ja verkkodeformaation verteksikoordinaattien optimointi.
* wgpu / WGSL Compute Shaders: Monisäikeinen ja GPU-kiihdytetty renderöinti. Muunnokset ja easing-laskenta hajautetaan suoraan näytönohjaimelle tween.wgsl-laskentashaderin kautta 100 000+ yhtäaikaiselle entiteetille. Verteksideformaatiopuskurit jaettavissa suoraan rusty_skelform-rakenteista nollakopiona GPU-puskureille.

Rust-ytimen äärimmäinen suorituskyky ja laskentakapasiteetti liitetään saumattomasti macOS-käyttöliittymäkerrokseen ilman muistikopiointiviiveitä.

### 4.2 Swift / SwiftUI / Metal: macOS-natiivi käyttöliittymä ja renderöintikanvas

Käyttöliittymäkerros on rakennettu macOS-natiivisti Swift 6- ja SwiftUI-teknologioilla. SwiftUI vastaa deklaratiivisesta hallintakomponentistosta, ikkunoinnista ja reaktiivisista tilapäivityksistä.

Renderöintikanvas perustuu Apple Metal -teknologiaan, joka hostaa Rust-ytimen wgpu-kontekstia CAMetalLayer-kerroksen läpi hyödyntäen wgpu-hal-abstraktiokerrosta. Tiedonsiirto Rust-renderöijän ja Metal-kanvaksen välillä toteutetaan nollakopio-periaatteella (Zero-Copy Buffer Sharing) suorien muistiosoittimien kautta ilman tavukopiointia, mikä takaa 60/120 FPS viiveettömän reaaliaikaisen esikatselun.

Metal-pohjainen esikatselukanvas saavuttaa täyden reaaliaikaisuuden, kun taas raskaat offline-puhe- ja ääniprosessointitehtävät delegoidaan taustalla toimivalle Python-alijärjestelmälle.

### 4.3 Python-alijärjestelmä: Ääniprosessointi, TTS ja aikaleimaus

Taustalla toimiva Python-alijärjestelmä huolehtii raskaasta offline-ääniprosessoinnista ja synteesistä. Järjestelmä ajaa local TTS -malleja (Kokoro/Piper) sekä suorittaa WAV-äänitiedostojen spektri- ja amplitudianalyysin.

Prosessi erottelee ääniraidasta millisekunnintarkat foneemitilanteet, muuntaa ne 10 viseemin järjestelmään ja validoi strukturoidun JSON-datamuodon ennen sen välittämistä Rust-ytimen muistiin.

Esikäsitellyt äänitiedot ja aikaleimamatriisit siirretään Python-alijärjestelmästä Rust-ytimelle suorituskykyisen prosessien välisen tietoliikenteen kautta.

### 4.4 Järjestelmien välinen IPC ja tiedonsiirto

Eri kielellä toteutettujen järjestelmien välinen tiedonsiirto on suunniteltu matalaviiveiseksi ja tyypitetyksi:

```text
┌─────────────────────────────────────────────────────────┐
│                    SwiftUI / Metal                      │
└──────────────────────────┬──────────────────────────────┘
                           │ C-FFI / Uniffi
                           │ (Nollakopio-muistinjako)
                           ▼
┌─────────────────────────────────────────────────────────┐
│                       Rust Core                         │
│  (broadsheet, rust-rule-engine, animato, wgpu compute)  │
└──────────────────────────▲──────────────────────────────┘
                           │
                           │ std::process / IPC Pipe
                           │ (JSON-RPC / Strukturoitu data)
┌──────────────────────────┴──────────────────────────────┘
│                    Python Subsystem                     │
│               (Kokoro/Piper TTS, Audio)                 │
└─────────────────────────────────────────────────────────┘
```


* SwiftUI \leftrightarrow Rust Core: Direct C-FFI / Uniffi -rajapinta. Ohjauskomennot, hiirensyötteet ja muunnosmatriisit siirtyvät ilman sarjallistamisviivettä.
* Python Subsystem \rightarrow Rust Core: Toimii erillisenä prosessina (std::process), joka kommunikoi IPC-putkien (IPC Pipe / JSON-RPC) kautta lähettäen esikäsitellyt huuliosynkronointi- ja aikaleimamatriisit.

Monikielisen teknologiapinon matalaviiveinen IPC-arkkitehtuuri luo pohjan reaktiiviselle ja selkeälle käyttäjäkokemukselle.

## 5. UX/UI & PROGRESSIVE DISCLOSURE -ARKKITEHTUURI

### 5.1 Minimalistinen macOS-käyttöliittymäsuunnittelu

Kilsat Studion käyttöliittymäfilosofia torjuu perinteisten animaatio-ohjelmistojen sekavuuden. Työskentelytila on suunniteltu selkeäksi, minimalistiseksi ja rauhalliseksi visuaaliseksi ympäristöksi, joka nojaa tyyliteltyyn sanomalehtiaestetiikkaan:

* Värimaailma ja typografia: Murrettu off-white-paperitausta, syvät mustepinnat, serif-otsikkofontit, monospaced-koodietiketit sekä hillityt tehostevärit (faded/accent).
* Asettelu: Keskiössä on puhdas ja häiriötön renderöintikanvas. Kompleksiset säätimet ja muuttujat on kätketty sivupaneeleihin, jotka avautuvat reaktiivisesti kontekstin mukaan.

Käyttöliittymän visuaalinen minimalistisuus kytkeytyy suoraan Progressive Disclosure -arkkitehtuuriin, joka säännöstelee järjestelmän kompleksisuutta.

### 5.2 Progressive Disclosure -kerrokset (Peruskäyttö -> Kompleksiset Rigi- ja Sääntönäkymät)

Käyttöliittymä soveltaa Progressive Disclosure -periaatetta, jossa monimutkaisuus paljastetaan käyttäjälle asteittain kolmen selkeän tason kautta:

## 1. Taso 1: Käsikirjoitus & Esikatselu (Script & Canvas) Käyttäjä työskentelee ainoastaan MotionScript YAML -tekstieditorin, pääkanvaksen sekä toistosäätimien (Play, Pause, Scrub) parissa. Soveltuu narratiivin ja perusanimaation nopeaan luontiin.
## 2. Taso 2: Interaktiiviset Visuaaliset Ohjaimet (Visual Rigging & Triggers) Käyttäjälle avataan visuaaliset sivupaneelit viseemien tarkistukseen, ilme- ja pose-kytkimien testaamiseen sekä fysiikkaprofiilien (esim. Snappy, Bouncy) valintaan liukusäätimillä.
## 3. Taso 3: Syväarkkitehtuuri (Advanced RETE Graph, IK & Spring Math Editor) Edistyneille järjestelmäarkkitehdeille suunnattu näkymä. Avaa visuaalisen RETE-sääntögraafin, luurankoverkon pistemuokkauksen sekä raakojen differentiaaliyhtälöparametrien (m, k, c) ja kustomoitujen loivennuskäyrien hienosäädön.

Progressiivisen käyttöliittymän eri kerroksissa määritellyt tilat ja syötteet ohjataan suoraan järjestelmän lopulliseen renderöinti- ja vientiputkeen.

### 5.3 Suorituskyky, deterministinen vienti ja renderöintiputki (FFmpeg / Frame Export)

Kilsat Studion vientiputki hyödyntää täysimääräisesti järjestelmän tilatonta determinismiä. Offline-renderöinnissä jokainen ruutu lasketaan absoluuttisen aikavälin pohjalta:

$$
t = \frac{\text{frame}}{\text{fps}}
$$

Koska laskenta on riippumaton reaalilajin suoritusajoista tai aiemmista kehyksistä, renderöintiputki tuottaa täysin bitti-identtisen lopputuloksen riippumatta siitä, ajetaanko se reaaliajassa vai tausta-ajona. Järjestelmä tukee useita vientimuotoja:

* FFmpeg-putkitus: RGBA-raakafraamien suora syöttö stdout-putken kautta ffmpeg-prosessille (esim. ProRes / H.264 / VP9).
* Sekvenssivienti: PNG-kuvasekvenssit alfa-kanavalla ja GIF-animaatiot.
* Synkronointi-JSON: Animaatiomarkkereiden ja aikaleimojen vienti narratiivin ulkoista synkronointia varten.

Tämä deterministinen offline- ja online-renderöintiputki vahvistaa Kilsat Studion arkkitehtuurin täyden valmiuden tuotantokäyttöön. Analyyttinen fysiikkalaskenta, RETE-UL-sääntöindeksointi sekä tilaton aikajanarajapinta tarjoavat laadukkaan ja toistettavan pohjan nykyaikaiselle digitaaliselle 2D-animaatiotuotannolle ilman tekoälymallien mukanaan tuomaa epävarmuutta.

## 6. Toteutuksen rajaukset ja tarkistettavat oletukset

Tämä osio ohjaa agenttien työtä ja täsmentää alkuperäisen raportin tavoitteita.

- **No AI -rajaus:** animaation ajonaikainen ydin on sääntöpohjainen. Piper/Kokoro-puhe kuuluu erilliseen valinnaiseen ML-esikäsittelyyn. Täysin mallivapaa tila käyttää valmista WAV-tiedostoa ja annettua tai käsin korjattua ajoitusta. Kehitystyössä käytettävät tekoälyagentit eivät kuulu sovelluksen ajonaikaiseen moottoriin.
- **Determinismi:** erotetaan kohtauksen tilan toistettavuus, pikselien toistettavuus ja pakatun videotiedoston tavutason toistettavuus. GPU:n, liukulukujen ja koodekkien alustariippumattomuutta ei oleteta.
- **Sääntömoottori:** yleistä O(1)-suorituskykyä ei luvata. Agentti mittaa indeksoinnin, liitosten, sääntöjen laukaisujen ja muistinkulutuksen kustannukset erikseen.
- **Ajallinen tila:** tallennetut käyttäjä- ja kameratapahtumat ovat syötettä. Dynaamisia sääntöjä ei saa jättää piilotetun toistohistorian varaan. Käännetty tapahtumajälki tai määritelty toistettava palautusmenettely tarvitaan.
- **Fysiikka:** homogeeninen jousiyhtälö ei yksin kata mielivaltaista tuulta ja muuttuvia ulkoisia voimia. Pakotettu vaste tai deterministinen numeerinen menetelmä määritellään erikseen. Myös alkunopeus voi vaikuttaa ylitykseen kriittisessä vaimennuksessa.
- **Puhe:** amplitudi ei yksin anna tarkkoja foneemeja. Suomen ä, ö, y, pitkät vokaalit ja kaksoiskonsonantit sekä englannin foneemit tarvitsevat omat säännöt ja validointiaineiston. Lähteen kirjainlista on lähtöluonnos.
- **Kirjastot:** broadsheet-, rust-rule-engine-, animato- ja rusty_skelform-nimet ovat tarkistettavia ehdokkaita. API:t, lisenssit, ylläpito, PSD-tuki ja yhteensopivuus vahvistetaan ennen riippuvuuspäätöksiä.
- **Suorituskyky:** 60/120 FPS, 100 000 entiteettiä, nollakopiointi ja alle viiden minuutin tuotanto ovat mitattavia tavoitteita. Laitteisto, kohtaus, resoluutio ja aikaraja dokumentoidaan mittaustuloksen yhteydessä.

## 7. Agentit ja orkestroija

Tässä määritellään kymmenen toteutusagenttia ja yksi orkestroija. Nämä ovat uudelleen käytettäviä tehtävänantoja; kaikkia toteutusagentteja ei ole käynnistetty eikä sovellusta rakennettu tämän dokumentin luonnin yhteydessä.

Jokaisella agentilla on oma tehtävätiedosto (tarkistuslista, hyväksymisehdot ja lopputulema) kansiossa [`docs/agents/`](agents/README.md) ja Claude Code -aliagenttimäärittely kansiossa `.claude/agents/`.

### 7.1 Yhteinen toimintaohje

Jokaiselle agentille annetaan tämä dokumentti, projektin lähdekoodi kun se on saatavilla, tehtävän tunnus ja rajattu muutosalue. Agentti lukee projektin AGENTS.md-ohjeet ennen muutoksia. Jos koodia ei ole saatavilla, agentti tuottaa suunnitelman ja merkitsee toteutuksen estyneeksi. Se ei keksi olemassa olevia API-rajapintoja tai testituloksia.

Jokaisen agentin palautus sisältää: tavoite, muutetut tiedostot, rajapintavaikutukset, tehdyt tarkistukset ja tulokset, avoimet riskit sekä seuraavan työn riippuvuudet. Valmis-tila vaatii todennettavan tuotoksen. Rajapintamuutokset sovitaan orkestroijan kautta.

### 7.2 Vastuut ja kopioitavat tehtävänannot

| ID | Agentti | Tehtävänanto | Tuotos ja hyväksymisehto |
| --- | --- | --- | --- |
| A01 | Arkkitehtuuri ja riippuvuudet | Tarkista lähdekoodi, riippuvuudet ja lisenssit. Erota nykytila tavoitteesta. Määritä moduulirajat ja No AI -tilat. | Arkkitehtuuripäätökset ja riippuvuuslista; jokaisen riippuvuuden perustelu ja tarkistustila näkyvät. |
| A02 | DSL ja kääntäjä | Määritä versioitu käsikirjoitusskeema sekä seq-, par- ja stagger-semanttiikka. Validoi kohteet, ajat ja virheet. | Skeema, esimerkit ja kääntäjä; virheellinen syöte tuottaa paikannetun virheen. |
| A03 | Säännöt ja tapahtumat | Toteuta sääntöjen vakaa järjestys, prioriteetit ja konfliktien ratkaisu. Estä loputon ketjutus. Tallenna ulkoiset tapahtumat. | Sääntösopimus ja toistettava tapahtumajälki; sama syöte tuottaa saman laukaisujärjestyksen. |
| A04 | Aikajana ja determinismi | Ratkaise TrackSpec-radat, päällekkäisyydet, Rel/Abs/Revert ja ajan satunnaishaku. | scene_at_time-rajapinta; suora aikahyppy vastaa saman ajan normaalia toistoa sovitulla toleranssilla. |
| A05 | Riggaus, IK ja esineet | Säilytä tikku-ukkotyyli. Kiinnitä jalat lantioon ja kädet hartioihin. Toteuta nivelrajat, käden tartuntapisteet ja puhelin. | Rigi ja kiinnityssopimus; raajat ja esine pysyvät kiinni hahmon liikkuessa ja kameran vaihtuessa. |
| A06 | Fysiikka ja vedettävät raajat | Toteuta jousiratkaisut, alkuarvot, raajojen vapautus ja ulkoisten voimien määritelty käsittely. | Fysiikkamoduuli; raja-arvot ovat äärellisiä ja liike jatkuvaa myös vaimennussuhteen 1 lähellä. |
| A07 | Suomen ja englannin ääni | Määritä WAV-tuonti, valinnainen TTS, foneemiajoituksen alkuperä ja viseemien sekoitus. | Versioitu ääniaikajana; suomi/englanti-esimerkit ja käsinkorjattava synkka. |
| A08 | Renderöinti, kamera ja taustat | Toteuta kohtausgraafi, syvyysjärjestys, laaja- ja lähikuvat sekä yhtenäiset taustat tikku-ukkotyyliin. | Renderöintisopimus ja vertailukuvat; sommittelu ja käteen kiinnitetty puhelin toimivat eri kuvakulmissa. |
| A09 | macOS ja käyttöliittymä | Toteuta SwiftUI-kuori, turvallinen Rust-rajapinta ja kolme käyttöliittymätasoa. Rajaa muistien ja resurssien omistus. | Avattava macOS-sovellus ja käynnistysohje; toisto, pysäytys ja aikahyppy toimivat. |
| A10 | Vienti, CI ja laadunvarmistus | Toteuta ruutuaikaan perustuva vienti, äänen synkronointi, regressiotarkistukset ja suorituskykymittaus. | Toistettava vientikomento, CI ja tulosraportti; ruutumäärä, äänen kesto ja epäonnistuvan viennin käsittely tarkistettu. |

### 7.3 Orkestroijan tehtävänanto

> Toimi Kilsat Studion orkestroijana. Käytä tämän dokumentin agenttijakoa. Aloita projektin nykytilan tarkistuksesta ja tee tehtävärekisteri. Sovi A01:n kanssa moduulirajat ja yhteiset tietosopimukset ennen toteutusta. Anna jokaiselle agentille selkeä tavoite, lähtötiedot, sallittu muutosalue, riippuvuudet ja hyväksymisehto. Käynnistä rinnakkain vain toisistaan riippumattomat työt; anna yhdelle tiedostolle kerrallaan yksi kirjoittaja. Yhdistä tulokset pieninä kokonaisuuksina. Palauta epäonnistuva työ omistajalle ja kirjaa syy. Älä merkitse tehtävää valmiiksi pelkän raportin perusteella. Raportoi käyttäjälle toteutetut asiat, tarkistukset, estot ja seuraava vaihe. Älä väitä kaikkien ominaisuuksien valmistuneen ilman näyttöä.

Orkestroijan hallitseman tehtävärekisterin ehdotettu tiedosto on `master-progress.md`. Sen tilat ovat `todo`, `ready`, `running`, `review`, `blocked` ja `done`.

```yaml
# Tehtävärekisterin esimerkki, ei väite käynnissä olevasta työstä.
id: KS-001
owner: A01
status: todo
depends_on: []
objective: Selvitä nykyinen koodipohja ja vahvista moduulirajat
inputs:
  - arkkitehtuuriraportti
  - projektin lähdekoodi, kun saatavilla
outputs:
  - arkkitehtuuripäätökset
acceptance:
  - nykytila ja suunnitelma erotettu
  - riippuvuuksien tarkistustila kirjattu
evidence: []
blockers: []
```

### 7.4 Työjärjestys ja hyväksymisportit

1. **G0 – Nykytila:** A01 tarkistaa projektin, riippuvuudet ja rajaukset. Jos lähdekoodia ei ole saatavilla, toteutustyöt merkitään estyneiksi; dokumentointia voidaan jatkaa.
2. **G1 – Sopimukset:** A02–A04 sopivat SceneSpec-, TrackSpec- ja EventLog-rakenteista. A05, A07 ja A08 määrittävät rigien, äänen ja renderöinnin tarvitsemat kentät. Orkestroija hyväksyy yhtenäisen version.
3. **G2 – Pienin toimiva kohtaus:** kaksi tikku-ukkoa, yksi tausta, yksi puhelin kädessä, yksi kameravaihdos ja WAV-puhe. A04–A08 toteuttavat omat moduulinsa sovittujen sopimusten mukaan.
4. **G3 – Integrointi:** A09 liittää käyttöliittymän moottoriin. A10 tarkistaa koko ketjun käsikirjoituksesta videoksi. Äänen esikäsittely ei estä käyttöliittymän toimintaa.
5. **G4 – Julkaisuvalmiuden näyttö:** A10 raportoi testit, vertailukuvat, macOS-käynnistyksen, vientituloksen ja mitatun suorituskyvyn. Orkestroija kirjaa rajaukset ja hyväksyy vain osoitetut ominaisuudet.

Rajapintojen muuttaminen avaa niistä riippuvat tarkistukset uudelleen. Orkestroija keskeyttää epäonnistuneesta portista riippuvat työt ja jatkaa riippumattomia töitä. Rinnakkaisuus rajoitetaan käytettävissä olevaan agenttikapasiteettiin; kymmenen roolia ei edellytä kymmentä samanaikaista prosessia.

## 8. Ensimmäisen toteutuskierroksen tarkistuslista

- [x] Projektin lähdekoodi ja käynnistystapa tunnistettu. — ORK-G0; README `cargo run -p studio-desktop`.
- [ ] Riippuvuudet ja versiot vahvistettu; vaihtoehdot kirjattu puuttuville kirjastoille. — Osittain: A01-inventaario ja ADR-A01-07 (ORK-G0); Python-wheelien lukitus (KS-006) ja lisenssitekstit (KS-007) tekemättä.
- [x] SceneSpec-, TrackSpec-, EventLog- ja AudioTimeline-sopimukset versioitu. — ORK-G1 (PR:t #105, #106, #107, #109; korjaukset #113, #114).
- [x] Kaksi eheää tikku-ukkorigiä; jalat ja kädet kiinni vartalossa. — ORK-G2-02; `g2_rig` 6/6 (ORK-G4-01).
- [ ] Puhelin seuraa käden tartuntapistettä myös IK-liikkeessä. — Vain rig-tasolla (`g2_rig`); renderöidyssä kohtauksessa ei puhelinta (A08, KS-003c).
- [x] Laajakuva ja lähikuva säilyttävät oikean sommittelun. — ORK-G2-02; `g2_minimal_scene` 6/6 ja vertailukuvat (ORK-G4-01).
- [ ] Suomen- ja englanninkielinen ääniesimerkki sekä viseemien tarkistus. — Estetty: tarvitsee käyttäjän äänitteen ja käyttöluvan (A07, KS-002e).
- [ ] Aikahyppy ja tavallinen toisto antavat vastaavan kohtauksen. — Osittain: bitilleen vain `trackspec-v1`-kohtauksella; G2-kohtaus puuttuu (A04, KS-003d).
- [ ] Animaation ydin toimii ilman ajonaikaista ML-riippuvuutta. — Näyttö PR #123:ssa (hiekkalaatikko, `ModelPolicy::Strict`), ilman puhetta; rastitetaan yhdistämisen jälkeen.
- [ ] macOS-käynnistys ja videovienti tarkistettu. — Vienti: MP4 CI:ssä (PR #123); käynnistys: vain prosessi ~13 s, ikkunaa ei kuvattu (A09).
- [ ] CI-tulokset ja suorituskykymittauksen olosuhteet kirjattu. — Näyttö PR #123:ssa (`docs/evidence/g2-export`); rastitetaan yhdistämisen jälkeen.

Päivitetty 27.10.2026 näytön perusteella: [ORK-G4-01](agents/00-orkestroija.md#ork-g4-01--g4-näyttökooste-g4-ei-hyväksytty-27102026). G4 ei ole hyväksytty.

**Tämän toimituksen tila:** lähde on muunnettu Markdowniksi ja agenttien sekä orkestroijan tehtävänannot on laadittu. Ohjelmiston toteutus, yllä oleva tarkistuslista ja suorituskykylupaukset eivät ole tällä perusteella valmiita.


## 9. Tiivistelmä ja tämän työn rajaus — 26.10.2026

Tavoitteena on sääntöpohjainen, offline-käyttöön soveltuva 2D-animaatiostudio: käsikirjoitus käännetään editoitavaksi aikajanaksi, joka ohjaa tikku-ukkojen liikkeitä, ilmeitä, puhetta, käteen kiinnitettäviä esineitä, kameroita ja vientiä. Luku 6 täsmentää alkuperäisen raportin teknisiä oletuksia ja rajoittaa lukujen 1–5 ehdottomia lupauksia.

Tämä toimitus on dokumentointitehtävä. Se säilyttää liitteen, määrittelee kymmenen agenttiroolia ja orkestroijan sekä lisää vaiheistetun suunnitelman ja seurannan. Agenttiroolit ovat tehtävänantoja, eivät tässä toimituksessa käynnistettyjä toteutusprosesseja. Sovelluksen toteutusta ei aloiteta tämän dokumentin perusteella.

GitHub-kohde: `aleksipii/kilsatstudio`. Tarkistettu lähtöhaara: `main`, commit `b00fe965d0dafe0fd718d5d0ee36f012827e29f3`. Repositorion juuren `AGENTS.md` on luettu. Lähdekoodia ja ominaisuuksien valmiutta ei ole auditoitu tässä dokumentointityössä.

**Sovitus nykyiseen projektiin:** juuren agenttiohje määrittelee työpöytäkäyttöliittymäksi egui/eframe-ratkaisun. Tämän raportin SwiftUI/Metal-osuus on arvioitava arkkitehtuuriehdotus, ei päätös korvata nykyistä käyttöliittymää. A01 selvittää eron ennen A09:n toteutusta. Sama koskee raportin kirjastoja ja projektin nykyisiä crateja. Suomen- ja englanninkielisen puheen tavoite erotetaan nykyisen ohjeen English-first/Kokoro-lähtökohdasta. Projektin riippumattomuus- ja lisenssiohjeita noudatetaan.

## 10. Vaiheistettu toteutussuunnitelma ja tehtävärekisteri

Tämä rekisteri koskee tämän arkkitehtuuriehdotuksen jatkotöitä, ei koko repositorion historiallista valmiutta. Kaikki alla olevat toteutusvaiheet ovat aluksi `todo`. Dokumentin vienti ei muuta niitä valmiiksi.

| Vaihe | Omistaja | Riippuvuudet | Tehtävä ja hyväksymisnäyttö | Tehty | Tekemättä | Tila |
| --- | --- | --- | --- | --- | --- | --- |
| G0 / KS-001 | A01 + orkestroija | Ei | Kartoita olemassa oleva koodi, tarkista riippuvuudet ja lisenssit, ratkaise UI-linjauksen erot; tuota päätöslista. | Tavoite ja rajaukset dokumentoitu. | Koodikartoitus, riippuvuusauditointi ja päätösten vahvistus. | todo |
| G1 / KS-002 | A02, A03, A04; A05/A07/A08 kommentoivat | G0 | Sovi SceneSpec-, TrackSpec-, EventLog- ja AudioTimeline-versiot, virhekäytännöt ja ajankäsittely; yhteiset esimerkit hyväksytty. | Sopimusten vastuut määritelty. | Skeemat, semantiikka, yhteensopivuus ja hyväksyntä. | todo |
| G2 / KS-003 | A04, A05, A06, A07, A08 | G1 | Kaksi ehjää tikku-ukkoa, kädessä pysyvä puhelin, tausta, kameravaihto ja WAV; aikahyppyjen vertailu sekä kuva- ja ääninäyttö. | Pienimmän kohtauksen sisältö rajattu. | Toteutus, integraatio ja todennettu näytekohtaus. | todo |
| G3 / KS-004 | A09 + A10; moduulien omistajat korjaavat | G2 | Liitä hyväksytty käyttöliittymä, toisto, aikahyppy ja vienti; osoita koko käsikirjoitus–video-ketju sekä virhe- ja peruutustilanteet. | Integroinnin vastuut määritelty. | Käyttöliittymäkytkentä ja päästä päähän -tarkistus. | todo |
| G4 / KS-005 | A10 + orkestroija | G3 | Koosta CI, macOS-käynnistys, vienti, vertailukuvat ja suorituskykymittaus laite- ja versiotietoineen; hyväksy vain näytöllä. | Hyväksymisnäytön vaatimukset kirjattu. | Testiajot, laitevalidointi, mittaukset ja julkaisupäätös. | todo |

### 10.1 Orkestroijan vaihekohtainen työtapa

1. Lue tämän tiedoston luvut 6–10 ja repositorion soveltuvat `AGENTS.md`-ohjeet. Tarkista nykyinen `docs/master-progress.md` ennen jatkotöiden kirjaamista.
2. Anna agentille luvun 7.2 tehtävänanto sekä tehtävätunnus, tarkat syötteet, sallittu tiedostoalue, riippuvuudet ja hyväksymisnäyttö. Älä luo uusia moduuleja vain suunnitelman nimien vuoksi.
3. Merkitse tehtävä `ready` vasta riippuvuuksien valmistuttua, `running` työn alettua ja `review` tuotoksen valmistuttua. Käytä `blocked`-tilaa ja yksilöityä estettä, jos työ ei voi jatkua.
4. Salli rinnakkainen työ vain riippumattomissa tiedostoissa. Orkestroija ratkaisee yhteiset rajapinnat ennen muutoksia. Yhdellä tiedostolla on yksi kirjoittaja kerrallaan.
5. Tarkista tuotokset ja näytöt. Merkitse `done` vasta hyväksymisehdon täytyttyä; muuten palauta tehtävä omistajalle kirjattuine korjaustarpeineen.
6. Päivitä vaihekohtainen yhteenveto jokaisen luovutuksen, tarkistuksen, eston ja hyväksymisportin jälkeen. Säilytä avoimet työt myös vaiheen vaihtuessa.
7. Tee rajatut commitit ja tarkistettava pull request. Älä yhdistä päähaaraan ilman käyttäjän nimenomaista lupaa.

### 10.2 Pakollinen yhteenveto jokaisessa vaiheessa

Jokainen agentti palauttaa ja orkestroija kokoaa seuraavat tiedot. Käytä samaa muotoa käyttäjälle annettavissa välipäivityksissä ja projektin seurannassa.

```markdown
### [Vaihe / tehtävätunnus] — [päivämäärä]
- Tila: todo | ready | running | review | blocked | done
- Omistaja:
- Tehty: konkreettiset muutokset ja tiedostot.
- Tarkistettu: suoritetut tarkistukset, tulokset ja näyttölinkit.
- Tekemättä: avoimet tehtävät ja täyttymättömät hyväksymisehdot.
- Estot ja riskit: yksilöity syy sekä ratkaisemisesta vastaava rooli.
- Seuraavaksi: seuraava tehtävä, omistaja ja riippuvuudet.
```

## 11. Dokumentointitoimituksen vaihetiivistelmät

| Vaihe | Tehty | Tekemättä / jatko |
| --- | --- | --- |
| D1 — Lähde | Liite luettu; alkuperäinen raportti ja sen agenttijako säilytetty. | Teknisten väitteiden todentaminen kuuluu G0–G4-vaiheisiin. |
| D2 — Agentit ja orkestroija | Luvussa 7 kymmenen agentin tehtävänannot, tuotokset ja hyväksymisehdot; luvussa 10 tarkennettu työjärjestys ja raportointikäytäntö. | Toteutusagenttien käynnistäminen ja ohjelmistotyö odottavat erillistä jatkotehtävää. |
| D3 — Suunnitelma ja tiivistelmä | Luvut 9–10 sisältävät tiivistelmän, rajaukset, riippuvuudet ja tehty/tekemättä-seurannan. | Jokaisen toteutusvaiheen näytöt kirjataan työn edetessä. |
| D4 — GitHub-toimitus | Tämä tiedosto on toimituksen versionhallittava tuotos polussa `docs/KILSAT-STUDIO-ARKKITEHTUURI-JA-AGENTIT.md`. | Vienti vahvistetaan GitHubin commitilla, takaisinluvulla ja pull requestilla; päähaaraan yhdistäminen on erillinen toimi. |

**Valmiuden tulkinta:** dokumentaatio ja tehtävänannot eivät ole näyttö toteutetusta animaatiomoottorista, hyväksytyistä mittauksista tai käynnissä olevista agenteista. Tämän toimituksen muutokset rajoittuvat tähän Markdown-tiedostoon.
