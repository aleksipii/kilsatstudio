# A02 — Versionoitu käsikirjoitus ja kääntäjä

27.10.2026. Lähtö: main `496a8a4`. Omistaja: motion-script.
A01:n Rust/egui- ja moduulipäätökset säilyvät. Ei uusia riippuvuuksia,
verkkokutsuja, synteesiä tai kilpailevaa kohtauskelloa.

## Nykytila ja muutos

Aiempi `parse` lukee versionumerottoman Scriptin. Script::validate tarkistaa
hahmot, kohtaukset, koostumuksen, animaatioradat ja tapahtumat, myös päällekkäiset
move-tapahtumat. Säännöt ja automaattiohjaus ovat jo olemassa. Rakenteellinen
seq/par/stagger-kääntäjä ja sen versioitu syöte puuttuivat.

A02 lisää `compiler::compile(&str) -> Result<Script, Diagnostic>` sekä
`compile_with_origins(&str) -> Result<Compilation, Diagnostic>` samaan crateen.
Vanha parseri ja nykyinen Script/Scene/Event säilyvät. Tulos voidaan serialisoida
ja avata nykyisessä editorissa. Editorin suora v1-tunnistus ei sisälly muutokseen.

## Versio 1 ja skeema

Pakolliset juurikentät: `version: 1`, `script` (nykyinen Script) ja `schedules`
(kohtauksen tunnus → juurisolmu). Tyhjä schedules on sallittu. Muut versiot,
tuntemattomat kentät ja päällekkäiset YAML-avaimet hylätään.

[JSON Schema](../schemas/motionscript-schedule-v1.schema.json) kuvaa uuden kuoren,
solmut ja tapahtumalehdet. Sisäisen Scriptin normatiivinen tyyppiskeema on
`motion-script/src/lib.rs`, ja semanttinen tarkistin on Script::validate.
JSON Schema ei monista nykyistä laajaa rig-/koostumusskeemaa eikä korvaa
semanttista tarkistusta. Rikkova muutos tarvitsee uuden version ja migraation.

## Ajoitus

Ajat ovat äärellisiä f64-sekunteja kohtauksen alusta. Juuren aloitus S = 0.
Lapsen kesto on D_i ja järjestys on kirjoitettu lasten järjestys.

| Solmu | Lasten aloitus | Kesto |
| --- | --- | --- |
| seq, children | S + edeltävien kestojen summa | summa D_i |
| par, children | S | max D_i |
| stagger, delay, children | S + i × delay | max(i × delay + D_i) |
| wait, duration | ei tapahtumaa | duration |
| event, span, event | tapahtuma S + event.at | span |

Children ei saa olla tyhjä. Delay on vähintään nolla; duration ja span ovat
positiivisia. Event.at on suhteellinen siirtymä välillä [0, span).
Move/gesture-kesto mahtuu spaniin. Myös wait-solmujen täytyy mahtua kohtaukseen.
Kohtauksen kestoa ei kasvateta. Ylivuoto ja nollakestoksi pyöristyvä positiivinen
väli hylätään. Tarkistetaan sekä paikallinen että absoluuttinen aika.

Emotion käyttää spanin, vaikka itse muutos on hetkellinen. Speak tarvitsee
tekijän antaman spanin: kääntäjä ei mittaa WAV:ia, arvaa puheen kestoa eikä muuta
äänen nopeutta. Span varaa rakenteellisen ajan, ei leikkaa ääntä. A07:n pitää
verrata mitattua ääntä suunniteltuun aikaväliin ennen vientiä.

Tapahtumat lajitellaan vakaasti alkuajan mukaan. Tasatilanteessa tulevat ensin
Scriptin alkuperäiset tapahtumat, sitten puun syvyyssuuntainen kirjoitusjärjestys.
Saman hahmon päällekkäiset liikkeet hylätään nykyisellä validoijalla. Muut
kanavakonfliktit säilyttävät nykyisen arvioijan semantiikan; A02 ei lisää prioriteetteja.

## Virheet ja resurssirajat

Diagnostic sisältää code, path, message ja valinnaiset line/column-kentät.
Koodit: syntax, schema, version, reference, time, unsupported, validation, limit.
YAML-syntaksivirheen rivi ja sarake ovat 1-pohjaisia. Kenttäpolun indeksit ovat
0-pohjaisia. Esimerkiksi tuntematon hahmo paikantuu polkuun
`$.schedules["opening"].children[1].children[0].event.actor`.
Semanttisten virheiden rivinumeroita ei arvata. Vanhan Scriptin virheessä polku
on `$.script` ja viesti nykyisen validoijan tarkenne; kaikkien vanhojen rig- ja
koostumuskenttien kenttätason paikannus ei sisälly A02:een.

Käännös on atominen. Rajat: 1 MiB lähdetekstiä, yhteensä 1024 aikataulusolmua ja
syvyys enintään 32 (juuri = 0); lisäksi YAML-parserin omat rajat.
Viitteiden rakenteellinen validointi ei tarkista tiedostojen olemassaoloa.

## A03- ja A04-rajapinnat

Compilation palauttaa `script: Script` ja `origins: Vec<Vec<String>>`.
`origins[scene][event]` on vastaavan lopputapahtuman lähdepolku myös lajittelun
jälkeen. Polku on sidottu lähderevisioon, ei pysyvä tapahtuma-ID.

- A03: compile ei kutsu expand_rulesia. Sääntölaajennus tehdään erikseen sen
  jälkeen. A03 omistaa prioriteetit, ulkoiset tapahtumat ja EventLogin. Jos A03
  lisää tai järjestää tapahtumia, sen täytyy ylläpitää uusi alkuperäjälki;
  origins kuvaa vain kääntäjän palautushetkeä.
- A04: käytä olemassa olevia Script/Scene/Event-tyyppejä ja kohtaussekunteja.
  A02 ei luo kilpailevaa TrackSpec-tyyppiä, muuta Rel/Abs/Revert-semanttiikkaa tai
  pyöristä ruutuihin. Näytekellomuunnos ja scene evaluation säilyvät nykyrajalla.
- Tasatilanteiden järjestys on osa v1-sopimusta. Uusi tapahtumalaji edellyttää
  yhteistä Event-, validointi- ja arvioijamuutosta.

## Esimerkit ja testaus

[Kelvollinen esimerkki](../examples/motionscript-v1/nested.yaml) tuottaa ajat
1, 1, 1.5 ja 3 sekuntia. [Virhe-esimerkki](../examples/motionscript-v1/invalid-target.yaml)
tuottaa reference-virheen polkuun `$.schedules["opening"].event.actor`.

```sh
cargo run -p motion-script --example compile_script < examples/motionscript-v1/nested.yaml > compiled.yaml
cargo test -p motion-script --all-targets
```

Kahdeksan integraatiotestiä kattavat sisäkkäisen ajoituksen, pisimmän haaran,
nollaviiveen, puheen spanin, vakaat tasatilanteet, lähdepolut, vanhan parserin
hyväksymän tuloksen, viitteet, virheelliset ajat, tarkkuuden menetyksen,
konfliktit, tuntemattomat kentät, kaksoisavaimet ja resurssirajat.
Todelliset tarkistustulokset ja avoimet portit ovat master-progressissa.
Editorin suora v1-avaus, A03 EventLog, A04:n uudet radat sekä A07:n mitattu
puheajoitus ovat erillisiä jatkotöitä.
