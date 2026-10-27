# A07 — Suomen ja englannin ääni

Tila: **ready** G1-osuudelle (näytekello `motion-audio`, AudioTimeline `motion-lipsync`, ORK-G1-04) · Portit: G1–G2 · Tehtävät: KS-002, KS-003 · Lähtö: commit `496a8a4`

## Tavoite

Määrittää WAV-tuonti, valinnainen TTS, foneemiajoituksen alkuperä ja viseemien sekoitus;
tuottaa versioitu ääniaikajana (AudioTimeline) suomi- ja englantiesimerkein sekä
käsin korjattava synkka. Toteuttaa A01:n tiukka mallivapaa polku.

## Lähtötiedot

- `crates/motion-lipsync/src/lib.rs` (`Viseme`, 13 tilaa; `phoneme_to_viseme`),
  `finnish.rs`, `english/`, `manual.rs`, `import.rs`, `rhubarb.rs`, `timing_eval.rs`
- `crates/motion-audio/src/` (PCM-kello, miksaus), `crates/motion-voice/src/`
  (`scene_audio.rs`: tuotu ääni ohittaa synteesin), `crates/motion-tts/src/`
- `services/kokoro-runtime/`
- [finnish-speech.md](../finnish-speech.md), [english-lipsync.md](../english-lipsync.md),
  [manual-mouth-timing.md](../manual-mouth-timing.md), [workstreams/alignment.md](../workstreams/alignment.md)
- [ADR-A01-03](../a01-architecture.md) (No AI -polut)

## Nykytila (koodista)

- 13 viseemiä: `Rest, Closed, TeethLip, TongueTeeth, TongueUp, Sibilant, Sh, R, A, E, I, O, U`.
  Raportin 10 viseemin taulukko on eri jako.
- Suomen ortografinen estimaatti (`kilsat-finnish-orthographic-estimator` v1) on olemassa;
  ajoituksen alkuperä (manual / estimated / forced) on mallinnettu.
- Tuotu WAV ohittaa synteesikutsun, mutta yhtenäistä pakottavaa No AI -politiikkaa ei ole.

## Sallittu muutosalue

- `crates/motion-audio`, `motion-lipsync`, `motion-tts`, `motion-voice` ja niiden testit
- `services/kokoro-runtime/` (aja palvelun testit muutosten jälkeen)
- Ei mallien latausta ilman käyttäjän suostumusta; ei pilvipalvelua.

## Riippuvuudet

- Edeltää: A01 (No AI -politiikan rajapintaehdotus), A04 (näytekello).
- Kuluttajat: A08 (suun tila), A09 (politiikkavalinta käyttöliittymässä), A10 (vienti).

## Tehtävät — tarkistuslista

- [ ] Päätä 10 vs. 13 viseemiä: säilytä nykyinen 13 ja kirjaa kuvaus raportin taulukkoon
      (ei rikkovaa muutosta), ellei orkestroija päätä toisin.
- [ ] Määritä `AudioTimeline` v1: lähde (tuotu / syntetisoitu + provider + revisio),
      näytetaajuus, kesto näytteinä, offset, rajaus, foneemi-/viseemivälit ja niiden alkuperä.
- [ ] Toteuta tiukka mallivapaa politiikka: tarkistetaan ennen providerin käynnistystä;
      puuttuva WAV → tyypitetty virhe; ei automaattista fallbackia.
- [ ] Testi: provider, jonka jokainen kutsu epäonnistuu; WAV-työnkulku toimii ilman kutsuja,
      myös projektin uudelleenavauksessa ja viennissä.
- [ ] Suomi: tarkista `finnish.rs` kattavuus ä, ö, y, pitkät vokaalit ja kaksoiskonsonantit;
      lisää säännöt ja testit puuttuville.
- [ ] Englanti: tuntematon foneemi epäonnistuu (säilytä); dokumentoi IPA-kattavuus.
- [ ] Viseemien sekoitus (siirtymä kahden suun välillä) ajan funktiona, testattu rajoilla.
- [ ] Esimerkit: yksi suomen- ja yksi englanninkielinen WAV + ajoitus (ei mallin tuottamaa,
      ellei käyttäjä erikseen hyväksy); käsin korjattu ajoitus tallentuu `manual`-alkuperänä.
- [ ] Älä kuvaa ajoitusta validoiduksi ilman mittausta oikeaa ääntä vasten.

## Hyväksymisehdot

- AudioTimeline on versioitu ja serialisoituva; alkuperä säilyy viennissä.
- Tiukassa polussa yksikään synteesi- tai tunnistinkutsu ei käynnisty (testi).
- Suomi/englanti-esimerkit ja käsinkorjaus toimivat; Rust- ja kokoro-runtime-testit läpäisevät.

## Lopputulema

Odotettu: AudioTimeline v1, pakotettu No AI -politiikka testeineen, suomen sääntötäydennykset,
viseemikuvaus ja esimerkit.

### Toteutunut lopputulema

_Ei vielä aloitettu._

## Tarkistukset

```sh
cargo test -p motion-lipsync -p motion-audio -p motion-voice -p motion-tts
python3 -m unittest discover -s services/kokoro-runtime/tests -v
```

## Kopioitava tehtävänanto

> Toimi agenttina A07. Lue `AGENTS.md` (Kokoro-osio), `docs/agents/README.md` ja
> `docs/agents/A07-aani-fi-en.md`. Älä lataa malleja ilman suostumusta. Tee
> tarkistuslistan tehtävät, aja tarkistukset ja päivitä tarkistuslista sekä
> Toteutunut lopputulema.
