# Agentti 03 — näyttely ja hahmoliike

Tila: **suunnitelma**, ei toteutettu automaattiohjaaja eikä hyväksytty laatutulos.
Tavoite on alkuperäisten tyyliteltyjen hahmojen luettava, motivoitu näyttely.
Pixarin tai Disneyn tuotantolaatua, hahmoja tai tuotantotekniikkaa ei oleteta
saatavaksi tällä säännöstöllä. Kokonaisuus: [automaation suunnitelma](../README.md).

## Nykyinen pohja ja puutteet

- `crates/motion-animation/src/clips.rs`: `HumanoidState`, `ClipWeights`,
  `evaluate_humanoid_with_weights_and_offsets`, `gesture_envelope` ja
  `HUMANOID_BONES` muodostavat olemassa olevan seitsemännivelisen perusrigipolun.
  Kerrosjärjestys on tunneposeeraus → kävely → enintään yksi ele → kirjoitetut
  niveloffsetit. Eleet ovat `wave`, `point`, `nod`, `shrug`; ne eivät ole yleinen
  näyttelykirjasto tai kontaktiratkaisija.
- `crates/motion-script/src/lib.rs`: `composition.joint_tracks`,
  `composition.gaze_tracks` ja `GazeKey` ovat olemassa olevia lähdemuotoja.
  Alkuperäisen rigin `gesture` torjutaan omalle rigille. Automaatio ei saa kiertää
  tätä rajoitusta generoimalla virheellistä tapahtumaa.
- `examples/acting-rig.yaml` näyttää oman rigin suuvariantit, ilmekartat,
  pupillit, siemenestä määräytyvän räpäytyksen ja katseraidan. Se on pieni
  tasavärinen nukke, ei hyväksytty koko vartalon tuotantohahmo.
- Puuttuvat tässä suunnitelmassa kuvattu tarkoituspohjainen elevalinta,
  kanavien omistajuuden ratkaisija, kontaktitarkistus sekä mitattu
  näyttelylaatu. Tässä työssä niitä ei ilmoiteta toteutetuiksi.

## Rajapinta

Syöte `DirectionPlan v1`: `version`, `seed`, 30–60 sekunnin kesto,
enintään kaksi hahmoa, vakaa `actor_id`, `rig_id` ja rigin sisältötiiviste,
aikakoodatut beatit, puhuja/kuulija, hyväksytty `intent`, korostushetki,
katseen kohde, kuvassa näkyvät vartaloalueet sekä prop-/jalkakontaktit.
Hahmopaketin kyvykkyyslista kertoo oikeat nivelet, ilmeet, suuvariantit ja
hyväksytyt eleklipit; vapaita nivelennimiä ei arvata.

Tulos `ActingPlan v1`: aikakoodatut `pose`, `gesture`, `gaze`, `expression` ja
`contact` -ehdokkaat, jokaisella `source_beat_id`, `rule_id`, `actor_id`,
`channel`, `joint_mask`, `start_sample`, `end_sample`, `priority`, `asset_hash` ja
valinnan perustelu. Kanoniset ajat ovat 24 kHz:n kokonaisnäytteitä; eleiden
ruutukvantit johdetaan niistä. Puheen ankkurit tulevat todellisesta näyteajasta. Puuttuva ääni antaa
luonnosajoituksen, jonka provenance pysyy `estimated`-tilassa.

Suunnitelma alennetaan nykyisen MotionScriptin tapahtumiksi ja
`composition`-raidoiksi, jotka nykyinen jäsentäjä ja validaattori hyväksyvät.
Uutta ruutukohtaista hahmosimulaattoria tai rinnakkaista kellomallia ei tehdä.
Tuntematon intent tai puuttuva kyvykkyys tuottaa lähdekohtaisen diagnostiikan.
Eksplisiittisen toiminnon puuttuminen estää lopullisen viennin; vain erikseen
sallittu valinnainen koristeliike voidaan jättää pois ja kirjata raporttiin.

## Säännöt

| ID | Ehto | Tuotettava liike ja rajaus |
| --- | --- | --- |
| ACT-01 | Beatilla on eksplisiittinen aikakoodattu toiminto | Toiminto omistaa ilmoitetut nivelet; automaattieleet eivät muuta niitä. Kaksi keskenään ristiriitaista eksplisiittistä toimintoa antaa virheen. |
| ACT-02 | `intent=explain` tai `demand`, korostushetki tunnetaan | Valitaan yksi hyväksytty avoimen käden tai osoituksen klippi. `demand` käyttää hahmon sallimaa amplitudia, ei automaattista karikatyyriä tai ylimääräistä huutoa. |
| ACT-03 | `intent=refuse` tai `set_boundary` | Vakaa poseeraus, yksi katseen palautus vastapuoleen ja tarvittaessa pieni pääliike. Aino saa olla vähäeleinen; joka lausetta ei pantomiimita. |
| ACT-04 | Eleklippi valittu ja tilaa löytyy | Esiliike 15 %, päätoiminto 60 %, asettuminen 25 % eleikkunasta. Ikkuna on oletuksena 0,6–1,8 s; vähintään yksi ruutu joka vaiheelle. Ajoitus mukautetaan puhekorostukseen eikä puhetta nopeuteta eleen vuoksi. |
| ACT-05 | Kuulijalle merkitty reaktiobeat | Katse → pää → ylävartalo, viiveet 2 ja 3 ruutua katseesta; alku vähintään 3 ruutua laukaisijasta. Enintään yksi merkittävä kuulijareaktio / 2 s. Ilman merkittyä beatia neutraali kuuntelu säilyy. |
| ACT-06 | Uusi katsekohde on kohtauksessa ja näkyvyys sallii | Katse siirtyy ennen päätä. Rigin oma katseraja pidetään; puuttuva silmäkyvykkyys ei synnytä keksittyä pupillia. Katsekohde-ID validoidaan. |
| ACT-07 | Puhe on aktiivinen | Suukanavan omistaa ääneen sidottu visemeraita. Tunneilme saa muuttaa kulmia ja poskia vain kyvykkyysmaskin kautta; se ei kirjoita suuta puhevisemen päälle. |
| ACT-08 | Ei semanttista toimintoa eikä kontaktiristiriitaa | Sallitaan hyväksytty vähäinen hengitys/idle-klippi. Enintään 1 pääele / 2 s ja yksi pääele aktiivisena; idle ei varasta luettavuutta. Räpäytyksiä ei sijoiteta koomisen reaktion tärkeimmille 3 ruudulle. |
| ACT-09 | Jalka tai käsi on kiinnitetty kohteeseen | Kontaktin nivelet ja niiden vaikuttavat esivanhemmat lukitaan. Ristiriitainen ele hylätään; käytetään ennakkoon hyväksyttyä kontaktiposeerausta. IK:ta ei väitetä nykytoiminnoksi. |
| ACT-10 | Klipin loppu tai beatin raja | Poseen jatkuvuus tarkistetaan rigin rajoilla. Ikkunaa lyhennetään vain, jos klipin hyväksytyt minimit säilyvät. Muuten valinnainen ele poistetaan tai pakollinen toiminto antaa virheen. |

Kaikki numeeriset oletukset ovat lähtöarvoja pilotin arviointiin, eivät mitattuja
luonnollisuuden rajoja. Hahmoprofiili voi kiristää niitä versioidusti.

## Omistajuus, sekoitus ja toistettavuus

Yhteinen ratkaisujärjestys on kontaktin tekninen rajoite, eksplisiittinen
kirjoittajan toiminto, puheen suukanava, beatin päätoiminto, kuulijan reaktio,
idle. Kontaktin ja kirjoittajan toiminnon ristiriita raportoidaan: kontaktia ei
käytetä perusteena muuttaa käyttäjän toimintoa hiljaa.

Jokaisella kanava/nivelparilla valitaan suurin prioriteetti. Tasan menevät
valinnaiset ehdokkaat ratkaistaan leksikografisella avaimella
`(source_beat_id, rule_id, candidate_id)`; pakolliset ristiriidat ovat virheitä.
Satunnainen pieni vaihtelu käyttää erillistä avainta
`hash(seed, actor_id, beat_id, rule_version)`, joten yhden hahmon lisäele ei muuta
toisen räpäytyksiä. Haku/seek missä tahansa järjestyksessä tuottaa saman asennon.

Perusrigillä alennus noudattaa nykyistä kerrosjärjestystä. Omat offsetit ovat
lisäyksiä eikä niillä luvata korvaavaa omistajuutta: jos valitun klipin vaatimaa
kanavaa ei voi eristää, kyseinen proseduraalinen kerros vaimennetaan tuetulla
painoraidalla tai vienti estetään. Omaa rigiä ohjataan sen hyväksytyillä
nivelraidoilla/rigiklipillä, ei alkuperäisillä `gesture`-tapahtumilla.
Normalisoidut sekoituspainot summataan yhteen vain samassa korvaavassa kerroksessa;
additiivisia offsetteja käytetään vain hyväksytyillä maskeilla ja rajoilla.
Tarkistus estää kaksoisliikkeen (sama ele sekä geneerisenä että nivelraitana).

Jalkalukko vaatii juurenkin tarkistuksen. Jos valmis klippi ei säilytä annettua
maailmakontaktia, automaatio valitsee liikkumattoman tuetun poseerauksen tai
ilmoittaa pakollisen kontaktin mahdottomaksi. Todellista IK:ta, muodonmuutosta tai
jalattoman perusrigikuvan fyysistä realismia ei päätellä lukkomerkinnästä.

## Aino–Rane-pilotti

Rane sanoo “ihan pieni keikka” → `demand`, pieni hyväksytty käden liike; ei
karikatyyrisesti jokaiselle sanalle nykäystä. Aino katsoo puhelinta, sitten
Ranea → katseraidan kohde vaihtuu ennen pään liikettä. Ainon rajaa asettava
vastaus käyttää vakaata asentoa ja yhtä pientä painotusta. Kilsat-insertin aikana
näkymättömät näyttelyeleet jätetään generoimatta; puhe ja kontaktit jatkuvat
yhtenäisellä aikajanalla. Paluukuvan asento otetaan samalta ajanhetkeltä.

## Työbudjetti ja hyväksyntä

Ehdotettu toiminta-alue on enintään 64 beatia, 480 ele-ehdokasta ja 20 000
generoitua avainta per 60 sekuntia. Rajat validoidaan ennen allokaatioita.
Näyttelysuunnittelun alustava seinäkellobudjetti on 5 s osana koko ajon
300 s tavoitetta. Se ei sisällä assettien tekijätyötä tai mallilatauksia eikä ole
suorituskykymittaus. Ajan loppuminen palauttaa keskeneräisen projektin ja syyn;
se ei ilmoita ammattilaatuista animaatiota valmiiksi.

Toteutuksen hyväksyntään tarvitaan seuraavat testit:

1. Sama IR, assettitiivisteet ja siemen antavat identtisen `ActingPlan`-tuloksen;
   suora seek ja järjestys 0 → loppu → keskelle antavat saman näytteen.
2. Perusrigi ja oma rigi saavat vain tukemansa tapahtumat; puuttuva käsi,
   tuntematon katsekohde ja kielletty `gesture` antavat täsmälliset virheet.
3. Kahden beatin samaan käteen osuvat pakolliset toiminnot estävät viennin;
   idle ja kuuntelureaktio väistyvät näkyvästi raportoidulla syyllä.
4. Visemen ja tunneilmeen yhtäaikainen näyte säilyttää puhemuodon; vain tuetut
   kulma-/silmäkanavat vaihtuvat.
5. Käsi propissa ja jalat maassa eivät liu'u juuriraidan mukana. Rajan ylittävä
   klippi torjutaan; lukkomerkintä yksin ei ole hyväksytty kontaktitesti.
6. Esiliike/päätoiminto/asettuminen mahtuvat 24 ja 30 fps aikajanoille;
   mahdottoman lyhyt pakollinen ikkuna antaa virheen ilman piilotettua nopeutusta.
7. Alkuperäisellä Aino–Rane-pilotilla säilytetään renderöidyt vertailut,
   arvioidaan eleen luettavuus, katse, jalkaliuku ja koominen ajoitus vähintään
   kahden arvioijan avulla. Rakennetestit eivät korvaa tätä havaintoa.

Tämä dokumentti on valmis suunnittelupanokseksi. Sääntömoottorin toteutus,
tuotantohahmot, ääniperusteiset ankkurit, kontaktitarkistin, renderöity pilotin
laatuarvio ja todellisella Macilla mitattu viiden minuutin kokonaisajo ovat auki.
