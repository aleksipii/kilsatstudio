# A01 — Arkkitehtuuri ja riippuvuudet

26.10.2026. Tarkastettu lähtö: `main`, commit `773ac9417ee000668fd4f92750e25797a9f57e34`.
Tila: **review**. Tämä on lähdekoodiin ja manifesteihin perustuva arkkitehtuurikatselmus,
ei koko sovelluksen toiminnallinen hyväksyntä tai jakelulisenssien hyväksyntä.

## Tehty ja tekemättä

- Tehty: luettu juuren AGENTS.md, workspace-manifestit, lukitus, nykyinen riippuvuusauditointi ja arkkitehtuuriehdotuksen rajaukset; tarkastettu synteesiraja ja WAV-ohitus lähdekoodista.
- Tehty: [riippuvuuslista](a01-dependencies.md), [koneellinen inventaario](a01-dependency-inventory.json), alla olevat päätökset ja hyväksymisportit.
- Tekemättä: tarkat kolmannen osapuolen lisenssitekstit ja jakeluvelvoitteiden tarkastus, kohdekohtainen Cargo-graafi, Python-wheelien täydellinen lukitus, laite- ja suorituskykymittaukset sekä No AI -tilan pakottava toteutus.
- Rajapintavaikutus: dokumentointimuutos; julkisia Rust-rajapintoja, riippuvuuksia tai käyttöliittymää ei muutettu.

## Nykytila ja tavoite

| Alue | Lähdekoodista / manifesteista havaittu | Tavoite tai avoin näyttö |
| --- | --- | --- |
| Työpöytä | `apps/studio-desktop/Cargo.toml`: eframe 0.31.1, glow; macOS rodio | SwiftUI ei nykyinen kuori eikä hyväksytty uudelleenkirjoitus |
| Kohtaus | `motion-scene/src/lib.rs`: ValidatedScript, evaluate, EvaluatedScene, frame_count | Satunnaishaun ja toiston ekvivalenssin laajempi hyväksyntä A04:lle |
| Puhe | `motion-tts/src/lib.rs`: SpeechProvider ja Kokoro-prosessiraja; `local.rs`: Piper, järjestelmäpuhe | ML-esikäsittely valinnaiseksi; No AI -kiellon läpäisevä politiikka puuttuu |
| WAV | `motion-voice/src/lib.rs` ja `scene_audio.rs`: imported_audio ohittaa synthesize_cancellable-kutsun | Koko työnkulun malliton hyväksyntä ja puuttuvan WAV:n fail-closed-käytös |
| Moduulit | 14 workspace-moduulia; manifestien normaalien riippuvuuksien graafi syklitön | Ei tyhjiä motion-core/timeline-crateja vain suunnitelman nimien takia |
| Renderöinti | motion-render: valinnainen wgpu/Metal-ominaisuus; motion-export: resvg | GPU-tuki ei todista Metalin käyttöä työpöydän kaikissa näkymissä tai CPU/GPU-pariteettia |
| Sääntö- ja fysiikkakirjastot | Raportin neljä nimettyä ehdokasta puuttuvat lockista | RETE-, fysiikka- ja suorituskykylupaukset eivät ole tällä perusteella toteutettuja |
| Lisenssit | Oma koodi ilmoitettu MIT:ksi; inventointityökalu olemassa | Jokainen ulkoinen artefakti tarvitsee erillisen tarkistuksen |

## Arkkitehtuuripäätökset

**ADR-A01-01 — Nykyinen Rust/egui-rakenne on jatkotyön lähtökohta.**
SwiftUI-siirtoa ei tehdä A09:n tehtävänannon perusteella automaattisesti. Mahdollinen
siirto tarvitsee erillisen päätöksen, omistus- ja FFI-sopimuksen sekä mitatun hyödyn.

**ADR-A01-02 — Omistus säilytetään nykyisissä moduuleissa.**

| Omistaja | Vastuu ja rajapinnan sääntö |
| --- | --- |
| motion-script | Käsikirjoituksen tyypit, jäsennys ja validointi; ei UI- tai synteesikutsuja |
| motion-project / motion-art | Versionoitu dokumentti, resurssiviitteet ja grafiikka-aineisto; UI-tila erillään |
| motion-rig / motion-animation | Luusto, poseeraus ja käyrät; ei tiedostojen latausta renderöinnin aikana |
| motion-scene | Kohtauksen arviointi eksplisiittisestä ajasta; yhteinen esikatselulle ja viennille |
| motion-audio / motion-lipsync | PCM-näytekello, miksaus ja ajoituksen alkuperä; estimaatti ei mitattu foneemiajoitus |
| motion-tts / services/kokoro-runtime | Vaihdettava synteesiraja ja prosessieristys; ei pakollista ML-riippuvuutta arviointiin |
| motion-voice | Valmistelutyöt, WAV-tuonti ja puheen orkestrointi; palauttaa valmiin äänen/metatiedot |
| motion-render / motion-export | Arvioidun tilan piirtäminen ja rajattu vienti; eivät määritä kilpailevaa kohtauskelloa |
| motion-jobs | Rajatut taustatyöt ja resurssibudjetit |
| studio-desktop | Käyttäjän valinnat, dokumentin editointi ja työn käynnistys; raskas työ pois tapahtumasilmukasta |

Taulukko määrittää jatkotyön vastuut; se ei väitä jokaisen nykyisen kutsun jo täyttävän
kaikkia rajoja. Todelliset crate-riippuvuudet ovat riippuvuuslistassa. Esimerkiksi
motion-export riippuu nyt myös motion-voice- ja motion-tts-moduuleista. Tätä ei
kuvata synteesistä täysin irrotetuksi binääriksi. Mahdollinen erottelu tehdään
konkreettisen tarpeen perusteella, yhteisiä tietotyyppejä monistamatta.

**ADR-A01-03 — No AI erotetaan kahdeksi käyttöpoluksi.**

| Polku | Sallittu | Estettävä | Nykyinen näyttö |
| --- | --- | --- | --- |
| Tiukka mallivapaa | Hiljainen animaatio tai tuotu WAV, käsin annettu ajoitus, sääntöpohjainen arviointi/renderöinti | Kokoro, Piper, järjestelmä-TTS, tarkistamattomat tunnistimet, mallilataukset ja automaattinen fallback | WAV ohittaa synteesin; yhtenäistä pakottavaa tilaa ei löytynyt |
| Sääntöpohjainen animaatio + valinnainen paikallinen ML-esikäsittely | Käyttäjän valitsema Kokoro/Piper valmistelussa; valmis WAV ja ajoitus animaation syötteeksi | Synteesi ruudun arvioinnissa, piilotettu pilvipalvelu tai lataus | Provider- ja prosessirajat olemassa; koko polun hyväksyntä erikseen |

Offline ei tarkoita mallivapaata. Valmiiksi syntetisoidun WAV:n toistaminen ei suorita
mallia, mutta aineiston alkuperä on silti ilmoitettava. Tuotteen No AI -merkintä ei
saa väittää synteettistä lähdeääntä ihmisen äänittämäksi. Kehityksen AI-agentit eivät
ole sovelluksen ajonaikainen riippuvuus.

Toteutussuositus A07/A09: yksi eksplisiittinen projektin/työn politiikka, jonka
valmistelu tarkistaa ennen providerin käynnistystä ja mallien asennusta. Tiukassa
polussa puuttuva WAV tuottaa virheen. Tallennettu ajoituksen alkuperä säilyy viennissä.
Tämä on sopimusehdotus, ei väite olemassa olevasta asetuksesta tai API:sta.

**ADR-A01-04 — Determinismin kolme tasoa erotetaan.**
Sama syöte ja aika → sama kohtauksen tila on ydintavoite. Pikselipariteetti vaatii
renderöijä-/fontti-/alustakohtaisen näytön. Pakatun videon tavuidenttisyys vaatii
lisäksi koodekin ja asetusten kontrollin. 60/120 FPS, 100 000 entiteettiä ja viiden
minuutin tuotanto ovat mittaamattomia tavoitteita tässä tarkastuksessa.

**ADR-A01-05 — Riippuvuuden lukitseminen ei hyväksy lisenssiä.**
434 lock-paketin inventaario sisältää myös workspace-, testi-, build- ja
alustariippuvuuksia, ei 434 toimitettavaa kirjastoa. Suorille riippuvuuksille on
kirjattu käyttötarkoitus, välillisille riippuvuuden tuovat paketit. Epäselvä
lisenssi tai jakelutarve pysyy avoimena. Ehdokaskirjastoja ei lisätä tässä työssä.

## Todellinen tarkistusnäyttö

- Inventaario tuotettu: `python3 scripts/license_inventory.py --output docs/a01-dependency-inventory.json`.
- `python3 -m unittest discover -s tests -p test_license_inventory.py -v`: **16/16 läpäisi**.
- Pythonin tomllib-lukija: 14 workspace-moduulia, 15 suoraa ulkoista Rust-nimeä,
  434 lukittua pakettia; normaali workspace-riippuvuusgraafi syklitön.
- Cargo ei ole tämän ympäristön PATH:ssa: Cargo metadata, check, Clippy ja Rust-testit
  eivät ajettu. Sovellusta, puhesynteesiä tai fyysistä Mac-laitetta ei testattu.
- Asennettua Python-ympäristöä, valittuja FFmpeg/Piper-binaareja tai mallipainoja
  ei ollut tarkastuksen syötteenä. Lisenssien alkuperäislähteitä ei tarkistettu uudelleen.

## Seuraavat hyväksymisportit

1. Orkestroija tarkastaa moduulirajat ennen A02–A10:n yhteisiä rajapintamuutoksia.
2. A07/A09 toteuttaa tiukan politiikan; testi käyttää provideria, jonka jokainen
   kutsu epäonnistuu, ja osoittaa WAV-työnkulun toimivan ilman kutsuja. Puuttuva WAV,
   projektin uudelleenavaus ja vienti eivät saa kiertää kieltoa.
3. A10 todentaa puhtaan, mallittoman ympäristön esikatselun ja viennin, ilman
   mallilatauksia tai käynnistettyjä synteesi-/tunnistinprosesseja.
4. Jakelukohtainen tarkastus täydentää lisenssit, NOTICEt, Python-wheelien hashit,
   mallien/äänien ehdot ja todelliset kohdekohtaiset riippuvuudet.

A01:n katselmointituotos on olemassa; lisenssihyväksyntä, toteutuksen No AI -portti
ja julkaisuvalmius jäävät avoimiksi. Muiden agenttien tehtäviä ei merkitty valmiiksi.
