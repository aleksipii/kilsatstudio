# Käyräeditori ja onion skin

Kaksi animaattorin perustyökalua: liikekäyrien muokkaus graafisesti ja
viereisten asentojen haamukuvat näyttämöllä.

Koodi: [`studio-desktop/src/curve_editor.rs`](../apps/studio-desktop/src/curve_editor.rs),
[`studio-desktop/src/onion_skin.rs`](../apps/studio-desktop/src/onion_skin.rs),
`Easing::Bezier` kirjastossa [`motion-animation`](../crates/motion-animation/src/lib.rs)
ja `bezier(...)`-easing tiedostossa [`motion-script/src/timing.rs`](../crates/motion-script/src/timing.rs).

## Vapaa ajoituskäyrä: `bezier(x1, y1, x2, y2)`

Avainruudun lähtevä easing voi olla esiasetuksen (`linear`, `ease_in`,
`back(1.3)` …) lisäksi kuutiollinen Bezier-käyrä, kuten CSS:n `cubic-bezier`:

```yaml
actor_tracks:
  VEIKKO:
    - {at: 0.0, x: 0, easing: "bezier(0.3, -0.4, 0.6, 1.5)"}
    - {at: 1.0, x: 120}
```

- x on aika ja y eteneminen. Käyrä kulkee pisteiden (0, 0) ja (1, 1) kautta.
- `x1` ja `x2` ovat välillä 0–1, joten liike etenee aina ajassa eteenpäin.
- `y1` ja `y2` ovat välillä −2–3. Alle 0 tuottaa ennakoinnin, yli 1 ylilyönnin, joka palaa avaimeen.
- Päätepisteet ovat täsmälleen 0 ja 1. Käyrän aika ratkaistaan 48 askeleen
  puolitushaulla, joten tulos on deterministinen ja sama esikatselussa, aikahypyssä ja viennissä.
- Väärä muoto tai alue on virhe, joka kertoo oikean muodon, esimerkiksi
  `bezier(0.42, 0, 0.58, 1)`. Esiasetukset tallentuvat ennallaan.

## Käyräeditori

Hahmon sijainnin ja koon avainruutujen, luiden ja omien rigien nivelten
editoreissa on avoinna **Käyräeditori**:

- **Arvokäyrät** ajan funktiona kanaville X, Y, kierto, X-mittakaava ja
  Y-mittakaava. Jokaisella kanavalla on oma värinsä, ja näkyvät kanavat valitaan
  valintaruuduista. Käyrät piirretään moottorin omalla raitanäytteistyksellä,
  joten ne näyttävät täsmälleen sen, mitä toistetaan. Toistopää näkyy pystyviivana.
- **Avainten veto:** vaakasuunta muuttaa aikaa ruutuun tarttuen ja pystysuunta
  valitun kanavan arvoa. Siirto, joka järjestäisi avaimet uudelleen, toisi kaksi
  avainta alle ruudun päähän toisistaan tai veisi avaimen kohtauksen ulkopuolelle,
  hylätään, ja veto pysyy viimeisessä kelvollisessa kohdassa.
- **Monivalinta:** vaihto+napsautus. Valitut avaimet liikkuvat yhdessä. "Poista valitut" poistaa ne.
- **Kaksoisnapsautus** lisää avaimen lähimpään ruutuun. Uusi avain saa kaikkien
  kanavien arvot nykyisiltä käyriltä ja edellisen avaimen easingin.
- **Ajoituskäyrä:** napsauta kahden avaimen väliä. Laatikossa on välin
  ajoituskäyrä kahdella vedettävällä kahvalla sekä numerokentät x1, y1, x2 ja y2.
  Esiasetuspainikkeet ovat Tasainen, Pehmeä, Kiihtyy, Hidastuu, Kiihtyy ja
  hidastuu, Ylilyönti ja Pito. Kahvan veto muuttaa esiasetuksen sitä vastaavaksi
  Bezier-käyräksi. Kahvat tallennetaan kolmen desimaalin tarkkuudella.

Muokkaus muuttaa vain luonnosta. **Käytä** tarkistaa käsikirjoituksen ja tekee
kumottavan muutoksen kuten ennenkin, ja numeeriset kentät ovat yhä editorin alla.

## Onion skin

Näyttämön Hahmot-tilassa on rivi **Onion skin**:

- **ennen** ja **jälkeen**: 0–3 haamua kummallekin puolelle toistopäätä
- **ruuduittain**, jossa väli on 1–12 ruutua, tai **avainruuduittain**, jolloin
  haamut ovat lähimmissä tallennetuissa avaimissa (juuri-, luu- ja nivelraidat).
  Nykyinen avain jätetään pois.
- **vain valittu hahmo**

Haamut ovat ääriviivoja. Menneet asennot ovat punaisia ja tulevat vihreitä, ja
ne haalistuvat kauemmas mentäessä. Asennot lasketaan samalla kohtauksen
arvioinnilla kuin toisto, ja nykyisen ruudun kamera rajaa ne, joten liike näkyy
paikallaan pysyvää kuvaa vasten. Haamut piirretään vain, kun toisto on
pysähdyksissä. Asetukset ovat käyttöliittymän tilaa eivätkä tallennu projektiin.

## Tarkistukset ja rajoitukset

- Testit kattavat Bezierin lineaarisen erikoistapauksen, päätepisteet,
  monotonisuuden, ennakoinnin ja ylilyönnin sekä rajat. Lisäksi ne kattavat
  MotionScript-muodon ja virheilmoitukset, editorin käyrien yhtäpitävyyden
  kohtauksen arvioinnin kanssa, vedon ruutuihin tarttumisen ja järjestyksen
  suojan, avaimen lisäyksen arvon säilymisen sekä haamujen ajat ja asennot.
- Haamut on katsottu kuvana tikkuhahmon kävelystä ja vilkutuksesta. Ääriviivat
  osuvat hahmon päälle, ja kaaret erottuvat.
- Editoria ja haamuja ei ole käytetty sovelluksessa käsin hiirellä, eikä niitä
  ole testattu fyysisellä Macilla.
- Käyräeditori on avainraidoissa (juuri, luut ja nivelet). Klippien ja painokäyrien
  editorit näyttävät Bezier-käyrän nimellä "Oma käyrä" mutta muokkaavat sitä vain tekstinä.
- Avainten arvokahvoja (tangentteja) ei ole erikseen, vaan välin muoto säädetään
  ajoituskäyrällä. Molemmat avaimet saavat siis saman käyrän välille.
