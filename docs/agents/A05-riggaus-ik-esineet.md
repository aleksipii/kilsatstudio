# A05 — Riggaus, IK ja esineet

Tila: **ready** (G2, ORK-G1; omistaja `motion-rig`, ORK-G1-05) · Portti: G2 · Tehtävä: KS-003 · Lähtö: commit `496a8a4`

## Tavoite

Säilyttää tikku-ukkotyyli; kiinnittää jalat lantioon ja kädet hartioihin; toteuttaa
nivelrajat, deterministinen kahden luun IK, käden tartuntapisteet ja puhelin, joka
pysyy kädessä hahmon liikkuessa ja kameran vaihtuessa.

## Lähtötiedot

- `crates/motion-rig/src/lib.rs` (`Transform2`, `Affine2`, `Rig::evaluate`),
  `definition.rs` (custom rig, rigid parts, attachment bind, `evaluate_acting`)
- `crates/motion-animation/src/clips.rs` (`evaluate_humanoid`, 7 pistettä)
- `apps/studio-desktop/src/hand_props.rs` (`grip`, `grip_parts`, esineluettelo)
- `crates/motion-export/examples/stick_studio_assets.rs`, `examples/stick-studio/`
- [custom-rigid-characters.md](../custom-rigid-characters.md), [rig-outlines.md](../rig-outlines.md),
  [stick-studio-art-direction.md](../stick-studio-art-direction.md)

## Nykytila (koodista)

- Luuhierarkia, rigid-osat ja kiinnitysten bind-matriisit ovat olemassa.
- Käsiesineillä on paikallinen `grip`-nivel, mutta esineluettelo on `studio-desktop`-sovelluksessa.
- IK-ratkaisijaa tai nivelrajoja ei löytynyt.
- Raportin SkelForm/PSD-tuonti ja GPU-verkkodeformaatio eivät ole nykyistä koodia.

## Sallittu muutosalue

- `crates/motion-rig/src/**`, `crates/motion-animation/src/**` (ei A06:n fysiikkamoduulia)
- `apps/studio-desktop/src/hand_props.rs` vain, jos esinesopimus siirretään jaettuun
  crateen orkestroijan hyväksynnällä.
- Ei `rusty_skelform`-riippuvuutta ilman A01:n päätöstä.

## Riippuvuudet

- Edeltää: A01, A04 (aikajana).
- Sopii yhdessä: A06 (vedettävät raajat käyttävät IK:ta), A08 (renderöinti eri kuvakulmissa).

## Tehtävät — tarkistuslista

- [ ] Kirjaa kiinnityssopimus: mitkä nivelet ovat juuria (lantio), mitkä lapsia
      (hartiat → kädet, lantio → jalat), ja varmista testillä, että raajat pysyvät kiinni
      kaikissa nykyisissä klipeissä.
- [ ] Nivelrajat (min/max kulma) rig-määrittelyyn; validointi ja testit.
- [ ] Analyyttinen kahden luun IK (lakikosinilla), taivutussuunta, saavuttamaton kohde
      → venytetty suora raaja ilman NaN-arvoja; testit rajoilla.
- [ ] IK-kohde aikajanaraidaksi tai tapahtumaksi A04:n sopimuksen mukaisesti.
- [ ] Tartuntapistesopimus: esineen `grip`-nivel sidotaan käden päätenivelen maailmamuunnokseen.
      Päätä esineluettelon omistava crate (ehdokas `motion-rig` tai `motion-art`).
- [ ] Testi: puhelin pysyy kädessä (toleranssi pikseleinä) IK-liikkeessä, kävelyssä ja
      kameran vaihtuessa.
- [ ] Säilytä tikku-ukkotyyli; ei uutta piirtotyyliä ilman taiteellista päätöstä.

## Hyväksymisehdot

- Raajat ja esine pysyvät kiinni hahmon liikkuessa ja kameran vaihtuessa (testit).
- IK tuottaa äärelliset arvot kaikilla syötteillä; samasta syötteestä sama tulos.
- Nykyiset rig- ja stick-studio-testit läpäisevät.

## Lopputulema

Odotettu: rig- ja kiinnityssopimus, nivelrajat, IK-ratkaisija, jaettu tartuntapistesopimus
ja kahden tikku-ukon testikohtaus puhelimen kanssa A08:lle.

### Toteutunut lopputulema

_Ei vielä aloitettu._

## Kopioitava tehtävänanto

> Toimi agenttina A05. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A05-riggaus-ik-esineet.md`. Säilytä tikku-ukkotyyli. Tee tarkistuslistan
> tehtävät sallitulla alueella, aja työtilan tarkistukset ja päivitä tarkistuslista
> sekä Toteutunut lopputulema.
