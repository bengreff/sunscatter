# Interview 2026-10-29 (evening): next steps after the fidelity review

Questions from the lead after the FIDELITY review ([fidelity-review-2026-10-29.md](fidelity-review-2026-10-29.md)) and the SEQUENCE and PLUME-ALTITUDE agents. The owner's answers are given verbatim where they wrote their own. D117 records the decisions, D114 gains the pad abort, and the owner's question about the organization (D027's open question) settled D027 and added D118.

## Answers

1. **Foundations before D115's fidelity:** yes, all three of the review's §4 foundations come first. F1 (split and merge by construction), then F2 (one mass truth), then F3 (the flight software reads `Knowledge`).
2. **The Merlin's soot:** 0.5 % of the main nozzle's flow (E), as PLUME-ALTITUDE left it. The gas generator's exhaust leaves by its own pipe, which is not drawn.
3. **After a pad abort:** "Reset the ship fully, after all it needs to refuel. New launch." Once the engines are shut down, a fresh vessel of the same kind goes on the pad as loaded: full tanks, new engines, nothing staged. The clock runs on and is not reverted (D114).
4. **Stages already run in the systems screen's editor:** locked, so the player cannot draft a change that is then refused.
5. **The parachute q / DESIGN q flicker on the pad:** gone; closed.
6. **The first flights to fly end to end, after the foundations:** both Crew Dragon to the ISS and back, and the booster coming home.
7. **Who flies the booster after separation** (stage 2 is still burning): its own flight software, as the real one does. It runs the boostback, entry and landing burns on F3's layer and reads only what its sensors know. Later it becomes a ship-OS program the player can read and edit (pillar 4). This revises D115's "the player flies" for vehicles that fly themselves in reality.
8. **Dragon's rendezvous and docking:** the player chooses the level. "2 can be activated if wanted, or you can switch on 1 and only have auto burn completion, or you can go fully 3." The three levels:
   - autonomous, as the real Dragon (the crew can take over);
   - the player plans the phasing burns, the burns run themselves, and the player hand-flies the final approach;
   - fully manual.
9. **The order of the two flights:** in parallel, as two workstreams after the foundations.
   - Dragon: the capsule–trunk joint, entry, water, docking.
   - Booster: the vacuum relight, boostback, retropropulsion, landing (a landing zone first, the droneship later).
10. **How to run the foundations:** one strong agent at a time on the foundations. F1 comes first, then a fresh agent for F2, because both touch the vessel's state. A second agent takes the quick fixes in other files, and the lead reviews.

## The organization (the owner's question, D027)

The owner: "The player needs to realistically have access to as many rocket parts as possible while still being a company. Maybe a near future aerospace merger?" The lead's brainstorm covered six real precedents:
- real product lines inside a company the player names;
- operating rockets bought from their makers (Arianespace with Soyuz at Kourou, ILS with Proton, Sea Launch with Zenit);
- building under license (Japan's Delta-based N-I and H-I; the RD-180's US production rights);
- reviving retired designs (Saturn V);
- lookalikes from D101's generators;
- buying launches.

11. **How the player gets hardware:** develop their own, buy from suppliers, license designs. "Buying a part from a supplier is simple enough, but the mechanics for building something yourself (which will eventually be the only way you build things on colonies) is a little more complex and must be ironed out. I would say we define a few thousand basic components, new ones for various tech levels, and they are relatively abstracted (not a single bolt, but rather a certain type of turbopump, not a specific brand of solar cell but rather a standard solar cell). Then we have entries for things like materials, time to construct, multi-purpose factories (for colonies), etc, and everything is built to work off a similar system on earth and on a colony except in a colony you need far more manufacturing from scratch. We don't need ANY of this now, it is a later roadmap goal. Save it." (D118.)
12. **The founding merger:** real companies at first, then: "it needs to be somewhat real, but also with elements abstracted away. Brainstorm how to handle things. Primarily you need to be able to launch iconic rockets. Actually, to be honest, it is possible that it would be better to be an aerospace startup. I don't know though." After the brainstorm: **a startup only.**
13. **Money:** contracts, selling services and parts, investors, government contracts.
14. **Routes to iconic rockets:** operating bought rockets, building under license, buying launches. "Also you can buy components from contractors, this is big. Be generous with this capability, like you can buy a full engine from another company." Reviving retired designs was not picked.
15. **Real companies:** real names, abstract: suppliers with their real products, terms and published roadmaps (E), no simulated management or stock, and no competition for contracts for now.

## Not asked (candidates for later)

The sea state for splashdown (a calm, flat sea first, marked E), the droneship's motion, what the booster's flight software shows the player while it flies, the ship OS's language.
