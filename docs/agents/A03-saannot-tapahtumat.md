# A03 — Säännöt ja tapahtumat

Tila: **ready** (EventLog `motion-script/src/event_log.rs`, ORK-G1-03) · Portti: G1 · Tehtävä: KS-002 · Lähtö: commit `496a8a4`

## Tavoite

Kirjata ja tarvittaessa täydentää sääntöjen vakaa järjestys, prioriteetit ja
konfliktien ratkaisu; estää loputon ketjutus; tallentaa ulkoiset tapahtumat
toistettavaksi tapahtumajäljeksi (EventLog).

## Lähtötiedot

- `crates/motion-script/src/rules.rs`, `rules_tests.rs` — `RuleTrigger`, `RuleCondition`,
  `RuleEffect`, `Script::expand_rules`, `MAX_RULES`, `RuleError`
- [animation-rules.md](../animation-rules.md), [audio-cues.md](../audio-cues.md)
- `apps/studio-desktop/src/cue_rule_editor.rs`, `webcam_pose.rs`, `direct_manipulation.rs`
- Raportin luvut 2.2 ja 3.4 sekä luku 6 (Ajallinen tila, Sääntömoottori)

## Nykytila (koodista)

- `expand_rules` on deterministinen: säännöt nimen mukaan nousevasti, laukaisut ajan ja
  kirjoitusjärjestyksen mukaan; tuotetut klipit `r_<sääntö>_<n>`.
- Vaikutukset ovat klippejä tai kamera-avaimia; ne eivät tuota uusia faktoja, joten
  nykyinen malli ei ketjuunnu.
- RETE-verkkoa, GRL-syntaksia tai yleistä EventLog-rakennetta ei ole.

## Sallittu muutosalue

- `crates/motion-script/src/rules.rs`, `rules_tests.rs`, [animation-rules.md](../animation-rules.md)
- EventLog-tyypin sijainti sovitaan orkestroijan kanssa (ehdokas: `motion-script` tai
  `motion-project`); ei uutta cratea ilman konkreettista vastuuta.
- Ei RETE-kirjaston lisäystä ilman A01:n päätöstä.

## Riippuvuudet

- Edeltää: A01, A02 (skeema).
- Kuluttajat: A04 (aikajana), A06 (vedettävien raajojen vapautus), A07 (viseemisäännöt),
  A09 (näppäin- ja kameratapahtumat).

## Tehtävät — tarkistuslista

- [ ] Kirjaa nykyinen sääntösopimus: järjestys, prioriteetti (sääntönimi), päällekkäisyys,
      rajat ja virheluokat.
- [ ] Päätä tarvitaanko eksplisiittinen prioriteettikenttä; jos kyllä, lisää se muuttamatta
      nykyisten skriptien tulosta.
- [ ] Jos ketjuuntuvia sääntöjä (vaikutus → uusi fakta) lisätään, aseta kiinteä
      ketjusyvyys ja laukaisumäärä; ylitys on tyypitetty virhe.
- [ ] Määritä `EventLog` v1: tapahtumatyyppi (näppäin, raahaus, kameran asento, vapautus),
      aika näytekellossa, lähde, arvo, versio. Ei piilotettua toistohistoriaa.
- [ ] Toteuta EventLogin serialisointi ja validointi (äärelliset ajat, järjestys, rajat).
- [ ] Testi: sama skripti + sama EventLog → sama laukaisujärjestys ja sama laajennettu skripti.
- [ ] Mittaa sääntölaajennuksen kustannus (säännöt × tapahtumat) ja kirjaa tulos laitetietoineen.
      Älä lupaa O(1)-suorituskykyä.

## Hyväksymisehdot

- Sama syöte tuottaa saman laukaisujärjestyksen (testi).
- Ketjutus ei voi jatkua rajatta (testi, jos ketjutus lisätään).
- Ulkoiset tapahtumat ovat syötettä, jonka toistaminen tuottaa saman kohtauksen.
- Nykyiset `rules_tests` läpäisevät muuttumattomina.

## Lopputulema

Odotettu: kirjattu sääntösopimus, `EventLog` v1 testeineen, mittaustulos ja
A06/A09:lle luovutettu tapahtumasopimus.

### Toteutunut lopputulema

_Ei vielä aloitettu._

## Kopioitava tehtävänanto

> Toimi agenttina A03. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A03-saannot-tapahtumat.md`. Säilytä `expand_rules`-järjestyksen
> nykyinen semantiikka. Tee tarkistuslistan tehtävät, aja työtilan tarkistukset ja
> päivitä tarkistuslista sekä Toteutunut lopputulema.
