# Weapon intent, equipment and CS profiles

`src/weapon.rs` owns reusable magazine timing and equipped weapon ownership.
`PlayerCommand` supplies intent; a composition chooses a game profile, collision
provider and presentation. None of the inventory types require a map or model.

## Implemented contract

- `WeaponSelection::Slot(u8)` requests a profile-defined group. Selecting the
  current group cycles its members in inventory order; another group selects its
  first member. A missing group does nothing.
- `Next` and `Previous` wrap through all entries. `Last` toggles the two most
  recently equipped entries; it does nothing before the first switch.
- `WeaponInventory<T>` is a body component containing nonempty, ordered
  `InventoryWeapon<T>` entries. Each entry owns its slot, timing configuration,
  magazine/reserve state and game-specific profile. `new` rejects an empty list.
- `select` returns whether equipment changed. A change cancels the outgoing
  reload without transferring ammo, preserves each weapon's magazine/reserve,
  and starts the incoming deploy lockout. The composition then applies any
  profile-specific deploy effects. Punch/recoil may live on the body and survive
  switching; inventory does not decide that policy.
- Only the equipped weapon advances its action timers. A composition calls its
  `WeaponState::tick` once per simulation step and consumes the returned
  `WeaponEvent` values. Automatic fire carries sub-step cadence remainder;
  semi-automatic fire consumes the trigger only when an attack actually occurs.
  Pressing during a lockout can therefore fire once when ready, until released.
- Times are seconds, ammo counts are rounds. Numbered slots have no global game
  meaning. Asset paths, pickup restrictions, shared ammo pools, damage, recoil,
  silencer/burst behavior and animation names belong to profiles/compositions.

The keyboard adapter buffers one equip request from `Update` into `FixedUpdate`:
digits 1–9 choose slots, Q chooses the last weapon, and brackets cycle. Mouse
wheel remains jump input. `secondary_fire` is controller-neutral held intent.

This is an equipment inventory, not yet a general pickup/item system. Entries
are fixed for the current session; mutable entry access must preserve their
configuration/slot identity. Ammo reserves currently belong to individual guns.

## CS composition

`CsGun` supplies AK-47, M4A1 and Desert Eagle profiles; `CsPunch` belongs to the
player. The FPS lab always has the selected starting AK viewmodel and adds M4A1
and Desert Eagle when their imported GLBs exist. Slot 1 cycles the two rifles;
slot 2 selects the pistol. This practice loadout intentionally permits both
rifles. It does not implement CS purchase, team, shield or primary-slot limits.

| Profile | Magazine | Cycle | Reload | Deploy | Movement maximum |
| --- | --- | --- | --- | --- | --- |
| AK-47 | 30 | 0.0955 s | 2.45 s | 0.75 s | 221 source units/s |
| M4A1 | 30 | 0.0875 s | 3.05 s | 0.75 s | 230 source units/s |
| Desert Eagle | 7 | 0.225 s | 2.2 s | 0.75 s | 250 source units/s |

Weapon selection updates the movement maximum before movement runs that tick.
Half-Life movement keeps its independent 320-unit maximum. M4A1 secondary fire
toggles its silencer with a two-second attack lockout and the imported attach/
detach sequence. Unsilenced/silenced damage is 32/33 with range factors .97/.95;
AK damage is 36 with .98, and Desert Eagle damage is 54 with .81. These factors
apply per 500 source units of travel. Hitscan range is 8192 units for the rifles
and 4096 for the pistol; world tracing uses `CollisionWorld::trace` with a point
hull. Practice targets use original box hit shapes without hitgroups or armor.

Presentation remains concrete in `glue/first_person.rs`: it preloads GLBs,
replaces the equipped model root, builds its named-animation graph and consumes
profile action names. Model generations discard stale asynchronous loads, so a
rapid equip sequence does not leave multiple visible guns. Imported clip
durations control playback independently of readiness/reload timers. Controllers,
movement, inventory and gun profiles can be reused without this renderer. The
current `FpsMap`/`FirstPersonGamePlugin` world-loading seam is still GoldSrc
specific; cross-game composition owns its eventual backend adaptation.

## Local behavior evidence and limits

Observed on the installed symbol-bearing `cstrike/cs.so` in Ghidra project
`mashup`, 2026-10-03. Timing is taken from attacks/reloads and common deploy;
skeletal sequence names/durations come from each installed MDL import catalog.

| Behavior | Function/address |
| --- | --- |
| M4 fire, integer cubic accuracy and stance recoil | `CM4A1::M4A1Fire`, `00125950` |
| M4 spread and cycle | `CM4A1::PrimaryAttack`, `00125ee0` |
| M4 reload/deploy | `CM4A1::Reload`, `00125540`; `Deploy`, `001255e0` |
| M4 silencer toggle/lockout | `CM4A1::SecondaryAttack`, `001256e0` |
| M4 movement maximum | `CM4A1::GetMaxSpeed`, `001254c0` |
| Desert Eagle accuracy, damage and kick | `CDEAGLE::DEAGLEFire`, `0011b680` |
| Desert Eagle stance spread/cycle | `CDEAGLE::PrimaryAttack`, `0011baa0` |
| Desert Eagle reload/deploy | `CDEAGLE::Reload`, `0011b3e0`; `Deploy`, `0011b450` |
| Rifle recoil increments and direction-flip probability | `CBasePlayerWeapon::KickBack`, `00105b00` |

Rifle accuracy uses integer division of shot-count cubed. Recoil uses the base
increment on shot one and the full shot count for subsequent increments; lateral
punch follows source yaw sign after coordinate conversion. Direction-flip
probabilities are observed, but the generator sequence is still approximate.

Exact Windows/runtime equivalence remains to be measured. Bullet sampling,
animation-variant selection and recoil RNG are approximate. Penetration, armor,
hitgroups, underwater attack restrictions, automatic empty-magazine reload,
source idle scheduling, audio/animation events and viewmodel bob are unfinished.
No source binary, decompiler output or imported game content is distributed.
