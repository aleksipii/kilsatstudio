# Jouset ja vedettävät raajat (`motion_animation::spring`)

Omistaja: A06 · Tehtävä: KS-003b · Koodi: `crates/motion-animation/src/spring.rs`,
testit `crates/motion-animation/tests/spring.rs`.

## Malli

Yksiulotteinen jousi `m·x'' + c·x' + k·(x − target) = F(t)`. Kaksiulotteinen liike tai
nivelkulma käyttää yhtä jousta komponenttia kohden. Aika on jousen paikallinen aika
sekunteina vapautushetkestä (`0..=1e6`); kohtausajan muunnos tehdään kutsujan puolella
(A04:n aikajana).

| Tyyppi | Tehtävä |
| --- | --- |
| `SpringParams` | `m`, `k`, `c` validoituna; `natural_frequency()` = ω_n, `damping_ratio()` = ζ, `regime()` |
| `Spring` | Suljetun muodon ratkaisu; `new`, `from_velocity`, `from_release`, `with_constant_force`, `sample(t)` |
| `ForcedSpring` + `ForceProfile` | Aikariippuva voima (vakio + `Gust` + `Turbulence`) kiinteällä askeleella |
| `SpringProfile` | Raportin taulukon profiilit vakioina |
| `release_state`, `ReleaseSample` | Raahauksen loppu → jousen alkutila |
| `SpringError` | Tyypitetyt virheet kaikille rajoille |

## Rajat (kaikki validoidaan, virheet tyypitettyjä)

- `m ∈ [1e-3, 1e3]`, `k ∈ [1e-3, 1e6]`, `c ∈ [1e-3, 1e6]` (äärellinen ja positiivinen).
- `|position|, |target|, |target + F/k| ≤ 1e6`, `|velocity| ≤ 1e8`, `|F| ≤ 1e9`.
- Aika äärellinen ja `0 ≤ t ≤ 1e6` s. Rajojen sisällä kaikki tulokset ovat äärellisiä
  (testattu ääripäiden ristikolla).

## Analyyttinen ratkaisu ja jatkuvuus ζ ≈ 1

Merkitään `a = c/2m`, `ω² = k/m`, `s = ω² − a² = (4km − c²)/4m²`. Sijoituksella
`y = e^{at}·(x − x_eq)` yhtälö on `y'' = −s·y`, joten

```
x(t) − x_eq = e^{−at}·( x0·C + (v0 + a·x0)·S )
v(t)        = e^{−at}·( v0·C − (ω²·x0 + a·v0)·S )
C = cos(√s·t),  S = sin(√s·t)/√s      (s > 0, alivaimennettu)
C = cosh(β·t),  S = sinh(β·t)/β       (s < 0, β = √−s, ylivaimennettu)
C = 1,          S = t                 (s = 0, kriittinen)
```

Kolme aluetta ovat saman lausekkeen tapauksia. Kun `|s|·t² < 1e-4`, `C` ja `S` lasketaan
Taylorin sarjasta (`1 − st²/2 + s²t⁴/24`, `t·(1 − st²/6 + s²t⁴/120)`), joten siirtymä
ζ = 1:n yli on jatkuva eikä kynnyshyppyä synny; `s = 0` antaa täsmälleen kriittisen
kaavan. Tämä korvaa tehtävälistan `|ζ − 1| < ε` -kynnyksen: kynnys on tulossa `s·t²`,
ei pelkässä ζ:ssa, jolloin myös pitkät ajat pysyvät tarkkoina.

Ylivaimennetussa haarassa eksponentit yhdistetään (`r1 = −ω²/(a+β)`, `r2 = −(a+β)`),
jotta `e^{−at}·cosh(βt)` ei muodosta `∞·0`:aa suurella ζ:lla.

Kriittinenkin jousi ylittää tasapainon, jos alkunopeus osoittaa tasapainoa kohti riittävän
voimakkaasti (`v0 < −a·x0`); testi `critical_damping_overshoots_with_strong_initial_velocity`.

## Ulkoiset voimat

- **Vakiovoima** (painovoima `F = m·g`): `with_constant_force(F)` siirtää tasapainon
  arvoon `target + F/k`; liike pysyy analyyttisena.
- **Aikariippuva voima** (`ForcedSpring`): kiinteä askel `1/1200 s`, joka ei riipu
  ruutunopeudesta. Jokainen askel soveltaa tarkkaa vaimennettua propagaattoria
  tasapainon `target + F(t_mid)/k` ympäri, missä voima pidetään askeleen keskipisteen
  arvossa. Menetelmä on stabiili kaikilla hyväksytyillä jäykkyyksillä (ei Eulerin
  räjähdystä). Viimeinen vajaa askel on täsmälleen `t − n·h`, joten tila on ajan funktio.
- Tila lasketaan aina ajasta: rakentaja tallentaa välitallennuspisteen joka sekunti
  (1200 askelta) samalla askelsarjalla, ja `sample(t)` jatkaa lähimmästä pisteestä.
  Tulos on bitilleen sama kuin askellus nollasta (`sample_from_start`, testattu), eikä
  riipu kyselyjärjestyksestä. Horisontti on enintään 3600 s ja annetaan rakentajalle;
  sen yli kysyminen on virhe `BeyondHorizon`.
- `Gust`: `A·sin(2π·f·t + φ)`, `f ≤ 100 Hz` (askel on 1200 Hz).
  `Turbulence`: siemenellinen arvokohina (SplitMix64-hajautus hilapisteisiin, smoothstep
  väliin), välillä `[−A, A]`, hilaväli `cell_seconds ∈ [1e-3, 1e3]`.

## Profiilit

| Profiili | m | k | c | ζ |
| --- | --- | --- | --- | --- |
| Snappy | 1.0 | 400 | 40 | 1.00 |
| Gentle | 1.5 | 100 | 15 | 0.61 |
| Bouncy | 0.8 | 250 | 8 | 0.28 |
| Stiff | 2.0 | 800 | 80 | 1.00 |
| Wobbly | 0.5 | 120 | 3 | 0.19 |

## Vapautussopimus

`motion-animation` ei riipu `motion-script`istä (ORK-G1-03). Raahaus- ja vapautustapahtumat
välitetään `motion-scene`n kautta tavallisina arvoina `ReleaseSample { time, position }`
(aika sekunteina, aidosti kasvava; muunnos EventLogin näytepaikoista tehdään kutsujan
puolella ORK-G1-04:n kellolla).

- Alkuasento = viimeisen näytteen asento.
- Alkunopeus = pienimmän neliösumman kulmakerroin näytteistä, jotka ovat enintään
  `window_seconds` (`(0, 1]` s) ennen viimeistä näytettä; yksi näyte → nopeus 0.
- Jousi alkaa paikallisesta ajasta 0 vapautushetkellä, joten asento ja nopeus ovat
  jatkuvia luovutuksessa (testi `release_has_no_jerk_at_the_handover`).
- EventLogin kytkentä odottaa KS-002f:ää (24 000 Hz -validointi); tämä moduuli ei lue
  lokia suoraan.

## Dangle-sopimus (ehdotus A05:lle, ei vielä sovittu)

- Toissijaista liikettä saavat vain rigin lehtinivelet tai erikseen merkityt lisäosat
  (hiukset, vaatteet, korvakorut, kädessä roikkuvat esineet), eivät IK-ketjun nivelet,
  jotta IK-ratkaisu ja jousi eivät kilpaile samasta vapausasteesta.
- Jousi ohjaa nivelen paikallista kulmaa additiivisena poikkeamana animoidusta asennosta;
  tavoite on animoitu kulma, joten levossa poikkeama on nolla.
- Nivelrajat (A05) leikkaavat lopputuloksen; jousen tila ei tiedä rajoista.
- Avoinna: merkintätapa rig-määrittelyssä (A05) ja säätimet käyttöliittymässä (A09).

## Rajoitukset

- Yksiulotteinen; ei törmäyksiä, ei kytkettyjä ketjuja (monilenkkinen hius) eikä
  epälineaarisia jousia.
- Aikariippuvan voiman ratkaisu on numeerinen (keskipistepito); virhe riippuu voiman
  muutosnopeudesta askeleen aikana, ei ruutunopeudesta.
- Visuaalista laatua ei ole arvioitu renderöidyssä kuvassa; testit ovat numeerisia.
