# Agentti 01 — käsikirjoituksen ymmärrettävä syöte

Tila: suunnitelma, ei toteutettu tuoteominaisuus. Tarkoitus on alkuperäinen,
tyylitelty animaatio. Pixar-/Disney-laatu ja alle viiden minuutin valmistuminen
eivät ole tämän dokumentin osoittamia ominaisuuksia.

## Todettu nykytila

`crates/motion-script/src/lib.rs` sisältää `parse(&str) -> Result<Script, Error>`,
`Script::validate`, `Script`, `Scene` ja `Event::{Move, Emotion, Gesture, Speak}`.
Syöte on englanninkielisesti nimettyä YAML-rakennetta; dialogiteksti voi olla
suomeksi. `Script` omistaa hahmot, kohtaukset, `animation_clips`, `rig_clips` ja
`rules`. Tavallisen suomenkielisen proosan tulkintaa tai alla kuvattua
`ScriptIntent`-sopimusta ei näissä tarkastetuissa tiedostoissa ole.

`crates/motion-script/src/rules.rs` sisältää `RuleTrigger`, `RuleCondition`,
`RuleEffect`, `Rules` ja `Script::expand_rules`. Nykyiset ehdot koskevat nimettyä
kohtausta, hahmoa, tunnetta ja aikaväliä; vaikutukset ovat klippi tai kamera.
Laajennus käyttää säännön nimen aakkosjärjestystä, laukaisuajan järjestystä ja
alkuperäistä tapahtumajärjestystä. Nykyinen toteutus ei ole yleinen proosan
merkitystulkki. Sen järjestystä tai virhekäyttäytymistä ei pidä korvata toisella
taustalla toimivalla sääntömoottorilla.

## Ehdotettu sopimus `ScriptIntent` v1

Tämä on suunnitteluvaiheen siirtomuoto ennen nykyistä MotionScript-kääntäjää,
ei uusi renderöitävä dokumenttimalli. Tarkka JSON Schema on toteutustehtävä.

| Kenttä | V1-sääntö |
| --- | --- |
| `version` | Täsmälleen `1`; tuntematon versio hylätään. |
| `seed` | Eksplisiittinen kokonaisluku 0…4294967295; sama syöte ja säännöstö tuottavat samat valinnat. |
| `duration_s` | Äärellinen kokonaiskesto 30…60 sekuntia. |
| `language` | `fi-FI`; ei lupaa puhepalvelun kielituesta. |
| `characters` | 1–2 vakaata tunnistetta, näyttönimet, käyttäjän valitsemat olemassa olevat asset-tunnisteet. |
| `scenes` | 1–3 kohtausta; yksikäsitteinen tunniste, olemassa oleva miljööviite ja positiivinen kesto. Kohtausten kestojen summa on kokonaiskesto. |
| `beats` | Enintään 64: kohtausviite, kohtauspaikalliset `start_s`, `end_s`, järjestysnumero ja `dialogue`/`action`/`hold`. |
| `dialogue` | Hahmotunniste, muuttumaton repliikki ja lähdeviite; ei automaattista uudelleenkirjoitusta. |
| `action` | Sallittu toiminto kuten `look_at`, `present_prop`, `react`, `idle`; kohde ja voimakkuus vain määritellystä sanastosta. |
| `source_span` | UTF-8-tavuväli `[start_byte,end_byte)` alkuperäiseen lähteeseen sekä lähdetekstin SHA-256. Rivinumerot ovat käyttöliittymän laskemia. |
| `provenance` | Jokaiselle päätelmälle sääntö-ID, sääntöpaketin versio ja `authored`/`derived`/`needs_review`. |

Aikavälit ovat puoliksi avoimia. Nolla-, negatiivinen tai ei-äärellinen kesto
hylätään. Suurin raakateksti on 64 KiB ja suurin dialogin kokonaismäärä 1000
Unicode-merkkiä. Raja on nopean profiilin esivalidointi, ei laadun tai puheen
mahtumisen takuu. Äänen todellinen kesto ratkaisee lopullisen ajoituksen.

## Säännöt ja virheet

| ID | Syöte → tulos | Epäselvä tai virheellinen syöte |
| --- | --- | --- |
| `ING-001` | Käyttäjän valitsema YAML → nykyinen `parse` ja `validate`. | Nykyinen jäsennys-/validointivirhe lähdekohtaan; ei proosaksi uudelleentulkintaa. |
| `ING-002` | Rajatun tekstimuodon `KOHTAUS`, `[alku–loppu]`, `AINO:` ja `RANE:` → intentin kohtaukset ja repliikit. | Tuntematon puhujan nimi tuottaa `UnknownSpeaker`, eikä kolmatta hahmoa lisätä. |
| `ING-003` | Suomenkielinen vapaa käsikirjoitus → irrotetut repliikkiehdotukset ja `needs_review`. | Moniselitteinen puhuja, pronomini, paikka tai toiminto tuottaa täsmäkysymyksen editorissa; ei hiljaista arvausta. |
| `ING-004` | `katsoo Ranea`, `näyttää puhelimen`, `tauko` → sallittu toimintotunniste ja lähdeviite. | Muut toiminnat säilytetään käsikirjoituksessa, mutta `UnsupportedAction` estää niiden esittämisen toteutettuina. |
| `ING-005` | Eksplisiittinen ajoitus → beatin lukittu aikaväli. | Ristiriitainen tai päällekkäinen dialogi samalla hahmolla hylätään; puhetta ei nopeuteta salaa. |
| `ING-006` | Hyväksytyt intentit → tavallinen MotionScript ja erillinen päätösraportti. | Puuttuva asset tai mahdoton sovitus estää käännöksen ennen ääni-/renderöintityötä. |

Prioriteetti on 1) käyttäjän lukittu tieto, 2) validoitu eksplisiittinen
merkintä, 3) valitun tuotantopaketin oletus, 4) yleinen sallittu oletus.
Saman prioriteetin eri tulokset tuottavat `IntentConflict`-virheen. Järjestys
stabiloidaan lähdevälin alun, beatin järjestysnumeron ja sääntö-ID:n mukaan.
Seediä saa käyttää vain samanarvoisten visuaalisten varianttien valintaan;
sen avulla ei ratkaista epäselvää dialogin merkitystä.

## Liitos nykyiseen putkeen

Intentin adapteri rakentaa `Script`-arvon nykyisiä julkisia tyyppejä käyttäen.
Puhe muuttuu `Event::Speak`-tapahtumiksi; perustellut tunne- ja elevalinnat
vastaaviksi nykyisiksi tapahtumiksi. Toiminnot, joille ei ole nykyistä esitystä,
ovat ominaisuuspuutteita, eivät ohitettavia merkintöjä. V1 voi tuottaa vain
tuetun hahmopaketin olemassa olevia klippi-/kamerasääntöjä.

Sen jälkeen suoritetaan nykyinen validointi ja `expand_rules`; syntyneet
tavalliset klippi-instanssit/kamera-avaimet toimitetaan samaan arviointiputkeen
kuin käsin kirjoitettu YAML. Lähdeviitteet pidetään adapterin sidecar-raportissa,
kunnes niiden pysyvä tallennus on erikseen toteutettu. Nykyisiä `RuleError`
luokkia `UnknownReference`, `Unsupported`, `Conflict`, `Bound` ja `Invalid`
ei typistetä onnistumiseksi. Tämän moduulin ei tule luoda animaation kelloa,
toista aikajanaa tai vaihtoehtoista suunliikekääntäjää.

## Pilottitapaus ja viiden minuutin raja

Aino–Rane-pilotin yksi 50 sekunnin toimistokohtaus käyttää kahta valmista
alkuperäistä hahmoa, yhtä miljöötä ja käsin tarkistettua Kilsat-kuvaa.
Repliikki `RANE: Tämä on ihan pieni keikka.` säilyy täsmälleen annettuna.
Toiminto `AINO näyttää puhelimen` tarvitsee nimetyn propin ja valmiin eleen;
ilman niitä ohjelma ilmoittaa puutteen. Kilsat-kuva ei saa väittää sovelluksen
ominaisuuksia, joita oikea tuote ei sisällä.

Tuonti+validointi+intenttien ratkaisu saa ehdotetussa 300 sekunnin
kokonaisbudjetissa enintään 5 sekuntia. Ajastin alkaa käyttäjän Käynnistä-
toiminnosta; epäselvyyden vuoksi pysähtynyt työ merkitään `needs_review`,
ei viidessä minuutissa valmistuneeksi. Vapaa proosa ei kuulu automaattisesti
hyväksyttävään nopeaan profiiliin ennen tulkinnan käyttäjähyväksyntää.

## Toteutuksen hyväksyntätestit

- Sama UTF-8-suomenkielinen teksti, seedi, asset-manifesti ja sääntöversio
  tuottavat tavutasolla saman normalisoidun intentin ja päätösraportin.
- Ääkköset ja moniriviset repliikit säilyvät; jokainen lähdeväli palauttaa
  oikean alkuperäisen tekstin. Muuttunut lähde hylkää vanhat spanit.
- Kolmas puhuja, 4. kohtaus, yli 60 s, NaN, ääretön aika, päällekkäinen
  saman hahmon puhe ja tuntematon toiminto antavat erilliset korjattavat virheet.
- Aino/Rane-järjestyksen tai dialogin merkitys ei muutu eri seedillä.
- Tuotettu MotionScript läpäisee nykyisen `parse`/`validate`-polun ja
  `expand_rules` on toistettava. Tuntematon klippi ei pääse renderöintiin.
- Ylikuormitustesti mittaa 64 KiB:n syötteen käsittelyn oikealla tavoite-Macilla;
  viiden sekunnin osabudjetin ylitys näkyy raportissa ja kokonais-SLA:n tuloksessa.
