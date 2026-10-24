# Paikallinen suomenkielinen puhe ja suunliikkeiden mittaus

## Toteutettu puhepolku

MotionScriptin suomenkielinen ääni käyttää language: fi-fi. Tuotantoon tarkoitettu
paikallinen Piper-polku käyttää provider: piper ja piper_-alkuista preset-tunnusta,
esimerkiksi piper_harri. Kokoro-polku pysyy englanninkielisenä ja hylkää suomen
ennen mallien tai ääniresurssien lukemista. LocalSpeechProvider ohjaa pyynnön
kielen ja preset-tunnuksen perusteella; projektiskeeman pitää tarkistaa myös
provider/preset-yhteensopivuus.

Sovellus ei asenna eikä lataa Piper-ohjelmaa, mallia tai ääntä automaattisesti.
Valitse erikseen asennettu paikallinen ohjelma ja tarkistetut malliresurssit.
Puheajo kuuluu taustatyöntekijälle. Syöte, lokit, WAV, kestot ja prosessin
suoritusaika ovat rajattuja; peruutus ja aikaraja tappavat ja odottavat prosessin.
Virhe ei vaihda toiseen kieleen eikä luo korvaavaa testiääntä.

Piper-profiili on JSON, jossa kaikki polut ovat absoluuttisia:

```json
{
  "executable": "/Users/example/piper-env/bin/piper",
  "model_path": "/Users/example/voices/fi_FI-harri-medium.onnx",
  "config_path": "/Users/example/voices/fi_FI-harri-medium.onnx.json",
  "cache_directory": "/Users/example/Kilsat/cache/piper",
  "preset": "piper_harri",
  "model_version": "pinned model revision and source identity",
  "environment_identity": "locally reviewed runtime/version declaration",
  "timeout_seconds": 120
}
```

Native-rate mono PCM16 WAV tarkistetaan ja muunnetaan deterministisesti 24 kHz:n
mono PCM16 -muotoon. Mallikonfiguraation espeak.voice-kielen pitää olla suomi.
Mallin, konfiguraation ja käynnistystiedoston todelliset SHA-256-tiivisteet
kirjataan. Version ja ympäristön merkkijonot ovat käyttäjän ilmoituksia, eivät
riippuvuusympäristön automaattinen auditointi.

Python-käynnistystiedoston tiiviste ei sido Piper/ONNX/eSpeak-riippuvuuksien
sisältöä. Siksi aiempaa pyyntökohtaista puhetulosta ei käytetä uudelleen:
Piper suoritetaan joka pyynnöllä ja cached on false. Vasta syntynyt, todelliseen
WAV-tiivisteeseen sidottu tulos deduplikoidaan muuttumattomaan sisältövälimuistiin.
Malli- ja ohjelmatiedostojen muuttuminen ajon aikana hylkää tuloksen. Korruptoitunut
oman sisältövälimuistin tiedosto tuotetaan uudelleen; lähdeäänitteitä ei korvata.

## Oikean äänen koesynteesi

```sh
cargo run --locked -p motion-tts --bin finnish_speech -- \
  --piper-config /absolute/piper-profile.json \
  --text-file /absolute/finnish-dialogue.txt \
  --report /absolute/new-speech-report.json \
  --allow-local-inference
```

Komento tuottaa oikean paikallisen WAV-tiedoston. Raportti sitoo täsmällisen WAV:n,
UTF-8-tekstin, resurssien tiivisteet, näytemäärän, äänen perustason mittaukset ja
kuluneen ajan. Reaaliaikakerroin on kulunut aika jaettuna äänen kestolla. Raportti
ei hyväksy äänen taiteellista laatua, suun ajoitusta tai julkaisuoikeuksia.
Koesynteesi edellyttää erillistä allow-local-inference-valintaa eikä lataa mitään.

## Macin järjestelmä-äänet ovat yksityistä esikatselua

SystemSpeechProvider voi listata jo asennetut suomenkieliset macOS-äänet ja
käyttää valittua täsmällistä nimeä. Puuttuva ääni tuottaa virheen. Asennettuja
ääniä ei oleteta olevan olemassa eikä englanninkielistä ääntä korvata suomen tilalle.
Pyyntökohtainen uudelleenkäyttö on poistettu myös tästä polusta, koska ääniresurssien
versiot eivät ole luotettavasti näkyvissä.

macOS 27:n lisenssin kohdan 2F perusteella järjestelmä-äänet rajataan tässä
sovelluksessa yksityiseen esikatseluun. Tätä polkua ei merkitä sarjan tai YouTuben
julkaisuääniksi. Käyttäjän voimassa olevat Apple-ehdot ja mahdollinen erillinen
sopimus on tarkistettava erikseen: [Apple macOS 27 SLA](https://www.apple.com/legal/sla/docs/macOS27.pdf).

## Harri-esimerkin tarkistettava lähde

Erikseen asennettavan Harri medium -mallin ensisijainen lähde on
[rhasspy/piper-voices](https://huggingface.co/rhasspy/piper-voices/tree/5041a7a49ae5b3a0f30e6837769eb03fdbb231bd/fi/fi_FI/harri/medium).
Tässä tutkittu muuttumaton versio on 5041a7a49ae5b3a0f30e6837769eb03fdbb231bd.

| Tiedosto | Tavua | SHA-256 |
| --- | ---: | --- |
| fi_FI-harri-medium.onnx | 63201294 | a44167faa34caed940e4fcad139fcc35922266b2593bcebe77701774c0fb2389 |
| fi_FI-harri-medium.onnx.json | 4873 | 3f9c9f76f74adf1fbe7279e41eea17d6610757e45effd6808bbea6be74b8916d |
| MODEL_CARD | 304 | a0a7fa3f046f1e582689db76f93099735530343a3126f3f40ed64380c419a0b2 |

Mallikortti kertoo suomen, yhden puhujan, 22050 Hz:n näytetaajuuden ja aineiston
CC0-merkinnän. Se kertoo myös jatkokoulutuksesta lessac medium -äänestä. Aineiston
CC0-merkintä ei yksin hyväksy kaikkia mallin, pohjamallin, puheohjelman tai
transitiivisten riippuvuuksien oikeuksia. Nykyinen
[OHF-Voice Piper](https://github.com/OHF-Voice/piper1-gpl) on erikseen asennettava
GPL-ohjelma; tätä toteutusta ei linkitetä siihen eikä sen ohjelmaa tai mallia
sisällytetä sovelluspakettiin. Lopullinen jakelu- ja aineistoarvio on avoin.

## Huulisynkan mitattava laatu

scripts/measure_lipsync_quality.py käyttää olemassa olevaa 24 kHz:n
suumuotomittaria. Syötteet sidotaan täsmälliseen WAV-tiivisteeseen, kieleen ja
näytemäärään. Ajat ovat yhden repliikin paikallisia, puoliavoimia 24 kHz:n
näytevälejä; ne eivät ole videon kuvaruutunumeroita.

```sh
python3 scripts/measure_lipsync_quality.py \
  --wav /absolute/real-fi.wav --language fi-fi \
  --inference-report /absolute/speech-report.json \
  --candidate /absolute/estimated-mouths.json \
  --reference /absolute/manual-mouths.json \
  --output /absolute/new-quality-report.json
```

Molempien suudokumenttien kentät ovat schema_version: 1, audio_sha256, language,
sample_rate: 24000, frame_count ja cues. Yksi cue sisältää start_sample,
end_sample ja shape-merkin A/B/C/D/E/F/G/H/X. Manuaalinen referenssi sisältää
lisäksi provenance: manual ja annotator. Ehdokas sisältää lisäksi source-arvon
estimated, text_estimated, estimated_rhubarb, forced tai manual sekä tool-tunnuksen.
Päällekkäiset välit, väärä kieli tai WAV, bool-arvot näyteaikoina, tuntemattomat
kentät ja epäkelpo JSON hylätään.

Raportti mittaa aktiivisten suumuotojen yhtäpitävyyden ja vertailukelpoisten
muotorajojen p95-aikavirheen millisekunteina. Oletusrajat ovat 0,75 ja 80 ms;
ne ovat valittuja mittausrajoja, eivät todistettu tuotantolaatumääritelmä.
Ilman oikeaan ääneen sidottua manuaalista referenssiä tila on unmeasured.
Testiäänellä tila on synthetic_only. Mittausrajojen epäonnistuessa tila on fail;
niiden täyttyessä ilman kuuntelu-/kuva-arviota tila on awaiting_review.
Ihmisen ilmoitettu arvio voidaan sitoa WAV:iin erillisellä review-JSON:lla,
mutta tuotantohyväksyntä säilyy not_granted. Tämä toteutus ei luo eikä väitä
omistavansa oikealla puheella annotoitua suomenkielistä laatukorpusta.

## Tarkistukset ja avoin hyväksyntä

Seitsemän Python-testiä tarkistaa mittaussemantiikan synteettisillä fixtuureilla.
Rust-testit kattavat palvelurajat, resamplauksen, peruutuksen/aikarajan,
virheellisen WAV:n, resurssi-identiteetin sekä välimuistin konservatiivisuuden.
Toteuttava agentti ei ajanut Rustia; integraattori kirjaa todelliset Cargo/CI-tulokset.
Oikean äänen koesynteesi, annotoitu referenssi, kuuntelu, Mac-laitetestaus,
suorituskyky sekä transitiiviset oikeudet tarvitsevat erilliset todisteet.
