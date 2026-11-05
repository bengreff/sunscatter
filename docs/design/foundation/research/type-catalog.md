# Part and component type catalog (research)

Status: research input for design session 1, not canon. Scope: a taxonomy broad enough to build any present-day launch vehicle, crewed capsule, station, satellite, probe, lander, or rover, plus near-future placeholders (NTR, NEP/SEP, fusion, sails) used only to pressure-test the taxonomy. This is not a database of real hardware.

Definitions in force (scope-2026-11.md 3.1): a **part** is the smallest placeable physical object: one shape, one rigid body, one exterior. A **component** is a parameter inside a part's shape that adds mass and drives the part's internal logic; it is not placed. Rule of thumb (3.1, 5.2): if a real program buys it from a supplier or swaps it between production runs, it is a component.

Field legend (scope-2026-11.md 5.1): F1 astrodynamics & control; F2 fluid dynamics & aerothermo; F3 thermodynamics & cryogenics; F4 chemistry & electrochemistry; F5 materials science & solid mechanics; F6 process science; F7 condensed matter & electronics; F8 electromagnetism & photonics; F9 plasma physics; F10 nuclear physics; F11 fundamental & particle physics; F12 planetary science; F13 astronomy & astrophysics; F14 life sciences; F15 computing & information.

Sources used throughout, cited briefly inline: NASA SP-8000 series (structures/loads design criteria), ECSS standards (European part/product discipline), SMAD (Space Mission Analysis and Design, subsystem chapters), NASA Systems Engineering Handbook, NASA/ESA part and ORU catalogs, Wikipedia for named real-hardware examples.

## 1. Part types

Columns: id, what it is, key shape parameters, mount faces/ports it typically exposes, component slots it typically holds, 2-3 real examples, science fields (F#) it draws on. ~100 types across 17 families (SMAD ch.10-21 subsystem split; NASA SP-8000 structures criteria; ECSS-E/Q discipline split for the structure/mechanism/thermal boundary).

### Structure

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| structure.interstage | shell joining two stages, separates at burnout | diameter, length, wall material/thickness, taper | fwd/aft mount rings, separation joint | separation_bolt, clamp_band, pyro_cutter | Falcon 9, Electron, Saturn V S-IC/S-II | F5, F1 |
| structure.fairing_half | payload fairing shell half, aero + jettison | ogive/biconic profile, diameter, length, acoustic lining | longitudinal split joint, base ring | separation_bolt, pyro_cutter, spring_pusher, acoustic_blanket | Falcon 9, Ariane 5, Atlas V PLF | F2, F5 |
| structure.payload_adapter | ring/cone carrying payload loads, hosts separation | bolt-circle diameter, height, load rating | LV interface ring, payload ring | clamp_band_marman, separation_spring, separation_bolt | Falcon 9 PAF, ESPA ring | F5, F1 |
| structure.thrust_structure | cone/cylinder carrying engine thrust into airframe | diameter, cone angle, material, engine pattern | engine mounts (xN), tank interface | gimbal_bearing_ring, load_cell, strut | Falcon 9 octaweb, Saturn V thrust structure | F5, F1 |
| structure.truss_strut | open lattice primary/secondary load member | length, cross-section, material, end fitting | two end mount faces | fastener_bolted_joint, damper_isolator | ISS truss segment, sat bus struts | F5 |
| structure.panel_sandwich | flat/curved honeycomb or composite skin panel | face sheet material, core density, thickness | edge mounts to frame | insert_potted, doubler_reinforcement | sat bus panel, CubeSat frame | F5, F6 |
| structure.ring_frame | circumferential stiffening ring inside a shell | diameter, section modulus, material | bonded/riveted to shell, hard points | fastener_bolted_joint | tank stiffening rings, fuselage frames | F5 |
| structure.equipment_bay | generic enclosure hosting electronics/power/small fluid parts | internal volume, envelope, shielding | structural mount, power/data/thermal ports | flight_computer_card, battery_cell_pack, dc_dc_converter, pcu, coldplate | sat avionics bay, Dragon trunk pallet, CubeSat bus | F7, F15, F5 |

### Tankage

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| tank.cylindrical | straight cylinder, domed/flat heads | diameter, length, wall thickness, head shape, material, pressure | polar ports, feed/vent/fill | pmd_screen, baffle_antislosh, pressure_transducer, vent_valve | Falcon 9 / Electron / Centaur tanks | F5, F2, F3 |
| tank.spherical | spherical vessel, best strength/mass | diameter, wall thickness, material | polar/equatorial ports | pmd_screen, pressure_transducer | Apollo SM helium spheres, Gemini tanks | F5 |
| tank.toroidal | donut tank fit around other structure | major/minor radius, wall thickness | feed ports around circumference | baffle_antislosh | Centaur toroidal-tank studies | F5, F2 |
| tank.conformal | non-axisymmetric tank fit to free volume | free-form shell surface, thickness map | as placed | pmd_screen, pressure_transducer | sat bus conformal prop tanks | F5, F6 |
| tank.pressurant_copv | composite overwrapped high-pressure vessel | diameter, liner material, overwrap layup, burst pressure, cycle life | fill/vent port(s) | pressure_transducer, burst_disk, fill_valve | Falcon 9 COPVs, ISS He/N2 COPVs | F5, F6 |
| tank.cryo_vacuum_jacketed | double-wall tank, insulated annulus for cryogens | shell geometry, MLI layers, vacuum annulus, boil-off target | fill/drain, vent, feed, instrumentation | mli_blanket, vacuum_getter_pump, cryo_valve, level_sensor_cryo | Centaur LH2 tank, Shuttle ET, Starship | F3, F5 |

### Propulsion, primary

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| engine.liquid_bipropellant | chemical engine, liquid fuel + oxidizer | chamber pressure, mixture ratio, expansion ratio, thrust class, cycle | tank feeds (xN), thrust mount, gimbal | turbopump_assembly, main_valve, igniter_torch, gimbal_actuator, injector_element | Rutherford, Merlin, RS-25 | F4, F5, F2, F3 |
| engine.liquid_monopropellant | catalytic decomposition engine/thruster | chamber pressure, catalyst bed, thrust class | single propellant feed, thrust mount | catalyst_bed, main_valve, heater_resistive_patch | hydrazine apogee kick motors | F4, F5 |
| engine.solid_rocket_motor | case-bonded/cartridge solid motor | grain geometry, propellant formulation, case material, nozzle | case mount, igniter port, nozzle exit | igniter_pyrotechnic, nozzle_throat_insert, gimbal_actuator | SRB, Star 48, Castor 30 | F4, F5 |
| engine.hybrid_rocket | solid fuel grain + liquid/gas oxidizer feed | grain geometry, oxidizer feed rate, case material | oxidizer feed, thrust mount | main_valve, igniter_torch, injector_element | SpaceShipOne hybrid motor | F4, F5 |
| engine.electric_hall_thruster | Hall-effect plasma thruster | discharge power, channel diameter, B-field topology, Isp | power in, xenon feed, thrust mount | hollow_cathode, power_processing_unit, magnet_coil | Starlink Hall thrusters, SMART-1 | F9, F7, F4 |
| engine.electric_gridded_ion_thruster | electrostatic ion thruster | grid gap/voltage, beam current, chamber geometry | power in, propellant feed, thrust mount | ion_optics_grid (structural), hollow_cathode, power_processing_unit | NSTAR (Dawn), NEXT, RIT-10 | F9, F7, F11 |
| engine.electric_mpd_vasimr | high-power magnetoplasmadynamic/VASIMR thruster (placeholder) | RF/magnetic coupling power, propellant, Isp range | power in (high), propellant feed, thrust mount | magnet_coil_hts, power_processing_unit | VASIMR VX-200 (ground test) | F9, F7, F3 |

### RCS

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| thruster.rcs_cold_gas | pressurized-gas RCS thruster | throat diameter, chamber pressure, propellant | gas feed, thrust mount | valve.solenoid | early sat N2 thrusters | F2, F5 |
| thruster.rcs_monopropellant | small hydrazine/green-monoprop RCS thruster | thrust class, chamber pressure, catalyst bed | propellant feed, thrust mount | catalyst_bed, valve.solenoid | Shuttle RCS, many GEO sats | F4, F5 |
| thruster.rcs_bipropellant | bipropellant maneuvering/RCS thruster | thrust class, mixture ratio, chamber pressure | fuel+ox feed, thrust mount | main_valve, igniter_hypergolic, injector_element | Dragon Draco/SuperDraco, Shuttle OMS | F4, F5 |

### Power

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| power.solar_array_rigid | flat rigid PV panel, fixed/gimballed yoke | area, cell type/efficiency, substrate, deploy axis | hinge/yoke mount (to a boom part), power out | solar_cell_photovoltaic, bypass_diode, slip_ring_rotary | ISS array panel, many GEO sats | F7, F5, F1 |
| power.solar_array_flexible | roll-out/fold-out thin-film array | stowed/deployed area, cell type | root port to a deployable_boom part, power out | solar_cell_photovoltaic, bypass_diode | ROSA, MegaFlex | F7, F5, F6 |
| power.battery_module_housing | structural box/mount for battery cells (cells are components) | envelope, mount pattern, thermal interface | power bus port, thermal port | battery_cell_pack, battery_management_unit, fuse_power | Dragon trunk battery pallet, sat battery box | F4, F3, F7 |
| power.fuel_cell_module | housing for a fuel-cell stack, reactant interface | envelope, reactant ports, coolant interface | H2/O2 feed, power out, coolant | fuel_cell_pem_stack, dc_dc_converter | Gemini/Apollo/Shuttle fuel cells | F4, F3 |
| power.rtg | self-contained radioisotope-to-power unit | isotope loading, couple count, output/decay curve | structural mount, power out, radiator interface | thermoelectric_module, radioisotope_heat_source | MMRTG (Curiosity, Perseverance), GPHS-RTG (Voyager) | F10, F3, F7 |
| power.fission_reactor_surface | compact fission reactor for surface/space power (placeholder) | thermal power, core geometry, shielding mass | coolant loop, power conversion interface, mount | reactor_core_fission, control_rod_drive, radiation_shield_mass | Kilopower/KRUSTY, SNAP-10A (historical) | F10, F3, F5 |

### Thermal

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| thermal.radiator_panel | flat/curved panel rejecting heat via loop or heat pipes | area, coating emissivity/absorptivity, tube/pipe layout | coolant loop in/out, structural mount | heat_pipe_fixed_conductance, radiator_coating_optical | ISS radiators, sat OSR radiators | F3, F7, F5 |
| thermal.deployable_radiator | folding/rollout radiator for high heat loads | stowed/deployed area, hinge count, loop interface | deployment hinge, coolant loop | heat_pipe_variable_conductance | ISS deployable radiators, NEP concepts | F3, F5 |
| thermal.droplet_radiator | liquid-droplet radiator for high specific power (placeholder) | droplet rate, collection area, working fluid | fluid loop, structural mount | droplet_generator, droplet_collector | lab demonstrators only | F3, F9, F5 |
| thermal.coldbox_cryo | insulated enclosure holding an instrument/line below ambient | envelope, insulation type, target temperature | structural mount, thermal strap/coolant | cryocooler_stirling, mli_blanket, thermal_strap_flexible | JWST ISIM, cryogenic dewars | F3, F7 |
| thermal.heatshield_ablative | fixed-geometry entry heatshield, ablative/charring material | outer mold line, material layup, char-depth margin | structural mount, sensor penetrations | ablator_tile, thermocouple_sensor, strain_gauge | Dragon PICA-X, Apollo/Orion AVCOAT | F2, F5, F3 |
| thermal.heatshield_reusable_tps | reusable tile/blanket TPS system | tile grid, material (RCC/ceramic), attachment | structural mount, gap-filler interfaces | tps_tile_ceramic, strain_isolation_pad, gap_filler | Shuttle orbiter tiles/RCC | F2, F5 |

### Avionics

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| avionics.flight_computer_unit | primary flight/guidance computer box | throughput, redundancy (TMR), rad-hardness, I/O count | structural mount, data bus (xN), power | flight_computer_card, memory_module_radhard, watchdog_timer | Falcon 9 flight computer, Orion VMC | F15, F7 |
| avionics.data_handling_unit | payload/subsystem data concentrator and recorder | channel count, storage, interface standards | data bus, power, sensor inputs | solid_state_recorder, data_bus_node, fpga_card | sat C&DH unit, probe CDH box | F15, F7 |
| avionics.power_distribution_unit | switches and protects power to loads | channel count, current rating, switching type | power bus in, power out (xN), data | relay_power_switch, fuse_power, current_sensor | PDU on most LVs/sats | F7, F15 |
| avionics.sensor_interface_unit | signal conditioning/ADC between analog sensors and bus | channel count, sample rate, resolution | sensor inputs, data bus | adc_card, signal_conditioning_card | flight test instrumentation units | F15, F7 |
| avionics.subsystem_controller | generic programmable controller (valves, heaters) | I/O count, control loop rate | data bus, actuator/sensor lines, power | fpga_card, flight_computer_card | thermal/fluid subsystem controllers | F15, F7 |

### Comms

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| comms.transponder_unit | RF transceiver box for telemetry/command/tracking | frequency band, power out, modulation/coding | RF to antenna, data bus, power | power_amplifier_rf, software_defined_radio_card | LV telemetry transponder, Small Deep Space Transponder | F8, F15 |
| comms.antenna_dish_parabolic | fixed/gimballed high-gain reflector | diameter, focal ratio, surface accuracy, gimbal range | RF feed, gimbal mount (to a gimbal_actuator_mount part) | rf_feed_horn | Voyager HGA, New Horizons HGA | F8, F5 |
| comms.antenna_omni_patch | low-gain omni/patch antenna | frequency, pattern, mounting | RF connector, structural mount | rf_feed_horn | CubeSat patch antennas, LV telemetry omnis | F8 |
| comms.laser_terminal_optical | optical comms terminal (near-term) | aperture, pointing accuracy, data rate | optical bench mount, gimbal, data bus | laser_diode_module, fine_steering_mirror, optical_detector_array | LLCD, TBIRD, Starlink laser links | F8, F15, F7 |
| comms.relay_crosslink_unit | inter-satellite RF/optical crosslink unit | band, range, pointing | RF/optical, data bus | software_defined_radio_card, laser_diode_module | TDRSS crosslink, Starlink laser crosslink | F8, F15 |

### GNC sensors and actuators

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| gnc.star_tracker | optical attitude sensor matching star fields | FOV, aperture, update rate, accuracy | boresight-aligned mount, data, power | optical_detector_array, baffle_stray_light, processing_card | Ball CT-602, Sodern HYDRA | F1, F8, F13 |
| gnc.sun_sensor | coarse/fine sun-relative attitude sensor | FOV, accuracy, analog/digital | structural mount, data, power | photodiode_array | coarse sun sensors on most sats | F1, F7 |
| gnc.imu_inertial | gyroscope + accelerometer package | gyro type (MEMS/RLG/FOG), bias stability, bandwidth | structural mount, data, power | gyroscope_element, accelerometer_element | Honeywell HG series, RLG IMUs | F1, F7 |
| gnc.horizon_sensor | Earth/planet horizon crossing sensor | FOV, spectral band, accuracy | structural mount, data, power | optical_detector_array | IR horizon sensors on GEO sats | F1, F8 |
| gnc.gnss_receiver | satellite-navigation receiver | channel count, dynamics limits, accuracy | antenna RF in, data, power | rf_frontend_card, processing_card | LV GPS units, ISS GPS receivers | F1, F8, F15 |
| gnc.radar_altimeter_lidar | ranging sensor for landing/rendezvous/altimetry | range, accuracy, beam pattern, type | aperture mount, data, power | rf_transceiver_card or laser_diode_module, optical_detector_array | Mars lander terminal descent radar, rendezvous lidar | F1, F8, F12 |
| gnc.reaction_wheel | spinning-rotor momentum-storage actuator | max momentum, max torque, rotor inertia, bearing life | structural mount, power, data | motor_winding, bearing_rotary_assembly, tachometer | Honeywell HR-series, RSI wheels | F1, F5, F7 |
| gnc.magnetic_torquer | electromagnet producing torque against planetary field | dipole moment, coil geometry, power draw | structural mount, power, data | coil_winding_core | torque rods on LEO cubesats/sats | F1, F8, F7 |

### Mechanisms and separation

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| mechanism.deployment_hinge | rotary joint deploying a panel/antenna, spring/motor driven | range of motion, torque/spring rate, latch type | two mount faces (stowed/deployed side) | torsion_spring, latch_mechanism, position_sensor, drive_motor | solar array hinge, antenna deployment hinge | F5, F1 |
| mechanism.deployable_boom | telescoping/coilable boom extending a sensor/antenna/array | stowed/deployed length, cross-section, material | root mount, tip mount for payload | drive_motor, position_sensor | magnetometer booms, STACER/Bi-STEM | F5, F6 |
| mechanism.separation_system | pyro/non-pyro device releasing two mated parts | release force, actuation type, shock output | two mating ports | separation_bolt_pyro, frangible_nut, pyro_cutter | stage separation systems | F5, F6 |
| mechanism.clamp_band_release | circumferential band joint with release (Marman-type) | band tension, release type | two ring mating faces | clamp_band_marman, bolt_cutter_pyro | payload adapter clamp bands | F5 |
| mechanism.robotic_arm | multi-joint manipulator for grapple/assembly/sampling | reach, joint count, payload capacity, end-effector type | base mount, end-effector port | joint_actuator_robotic, encoder_position, end_effector_tool | Canadarm2, OSIRIS-REx TAGSAM arm | F1, F5, F15 |
| mechanism.gimbal_actuator_mount | actuated mount letting an engine/antenna/instrument pivot | range of motion, torque, response rate | fixed base mount, moving payload mount | hydraulic_actuator or electromechanical_actuator, position_sensor | engine TVC mounts, antenna gimbal mounts | F1, F5 |
| mechanism.docking_latch_mechanism | captures and rigidizes a docking interface after soft capture | capture envelope, retraction force, alignment tolerance | docking interface port | capture_latch, load_limiter_strut, umbilical_connector | NASA Docking System, CBM latches | F5, F1 |

### Recovery and entry

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| recovery.parachute_canister | packed parachute system plus deployment device | canopy area/type, pack volume, trigger, riser length | structural mount, riser attachment | pilot_chute, mortar_pyro, riser_line, pressure_switch | Dragon drogue/main, Soyuz descent chutes | F2, F5 |
| recovery.airbag_landing_system | inflatable cushions absorbing touchdown load | bag count, inflation pressure, vent orifice | structural mount | gas_generator_inflator, vent_valve | Mars Pathfinder/MER airbags | F2, F5 |
| recovery.retro_landing_legs | deployable legs for propulsive landing | leg count, stroke, crush energy absorption | structural mount (xN), ground pad | crush_core_honeycomb, latch_mechanism, contact_sensor | Falcon 9 legs, New Shepard legs | F5, F1 |
| recovery.inflatable_decelerator_hiad | inflatable aeroshell for entry deceleration (near-term) | deployed diameter, torus count, fabric material | mount to payload | inflation_gas_bottle, torus_bladder, tps_flexible_blanket | NASA HIAD/LOFTID | F2, F5 |
| recovery.ballute | inflatable balloon+parachute hybrid drag device | deployed shape, material, inflation system | riser attachment | inflation_gas_bottle, riser_line | ballute concepts, not widely flown | F2, F5 |
| recovery.drogue_mortar | pyro/pneumatic device firing a pilot/drogue chute clear | muzzle velocity, charge type | structural mount | pyro_charge, piston | Apollo/Dragon drogue mortars | F5, F4 |

### Crew and habitat

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| crew.pressurized_module_shell | habitable pressure vessel (capsule or station module) | internal volume, shell material, hatch/window penetrations, pressure rating | hatches, docking port, umbilical, windows | life_support_rack (ref), pressure_relief_valve, window_pane | Crew Dragon capsule, ISS Destiny/Zvezda | F5, F14, F3 |
| crew.life_support_rack | ECLSS equipment housing (O2/CO2/humidity/water) | crew capacity, consumable rates, redundancy | air duct ports, water/waste lines, power, data | co2_scrubber, o2_generator, humidity_condenser, water_recovery_unit | ISS ECLSS racks, Orion ECLSS | F4, F14, F3 |
| crew.seat_couch | crew seat/couch with impact attenuation | occupant envelope, stroke, load limiting | structural mount, restraint attachment | load_limiter_strut, restraint_harness | Crew Dragon seats, Soyuz Kazbek couches | F14, F5 |
| crew.airlock_module | chamber for EVA egress/ingress, dual hatches | internal volume, depress rate, hatch count | two hatch interfaces, suit umbilical, mount | depress_valve, hatch_mechanism, suit_umbilical_port | ISS Quest airlock | F14, F5, F2 |
| crew.eva_suit | pressurized personal spacesuit | pressure rating, mobility joints, life-support duration | suit umbilical/docking collar | portable_life_support_pack, joint_bearing, visor_optical | EMU, Orlan, xEMU | F14, F5, F3 |
| crew.centrifuge_habitat | rotating habitat section for partial artificial gravity (placeholder) | radius, rotation rate, structural envelope | rotary bearing to non-rotating hub | rotary_bearing_large, slip_ring_rotary | proposed Mars-transit centrifuges (concept) | F14, F5, F1 |
| crew.radiation_shelter | heavily shielded crew refuge for solar particle events | shielding mass/material, volume | hatch to main habitat | radiation_shield_mass, dosimeter | ISS "safe haven" areas, proposed Mars storm shelters | F14, F11, F5 |

### Payload and science instruments

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| instrument.imaging_camera | visible/IR imaging instrument | aperture, focal length, detector format, spectral band | optical bench mount, data, power, thermal | optical_detector_array, filter_wheel, focal_plane_cooler | LORRI (New Horizons), HiRISE (MRO) | F8, F13, F12, F7 |
| instrument.spectrometer | dispersive/FTIR instrument measuring spectra | spectral range, resolution, detector type | optical bench mount, data, power | diffraction_grating, optical_detector_array, calibration_lamp | OSIRIS-REx OVIRS | F8, F12, F4 |
| instrument.mass_spectrometer | ionizes and separates sample species by mass | mass range, resolution, ionization method | sample inlet, data, power, vacuum interface | ion_source, mass_analyzer, electron_multiplier_detector | Curiosity SAM, Rosetta ROSINA | F4, F6, F11 |
| instrument.magnetometer | measures local magnetic field vector | sensitivity, range, boom offset | mount on a deployable_boom part, data, power | fluxgate_sensor_element | MAVEN MAG, Juno MAG | F8, F12, F13 |
| instrument.particle_detector | detects charged particles/radiation flux | energy range, species discrimination, geometric factor | structural mount, data, power | solid_state_detector, scintillator_element, photomultiplier_tube | Van Allen Probes ECT, crew dosimeters | F11, F10, F14 |
| instrument.radar_sounder | penetrating radar for subsurface/atmosphere sounding | frequency, bandwidth, antenna length | antenna deployment, data, power | rf_transceiver_card, antenna_dipole_deployable | MARSIS (Mars Express), SHARAD (MRO) | F8, F12 |
| instrument.sample_collection_arm | contact sampling mechanism for surface/asteroid material | reach, contact force, sample volume | structural mount, sample canister interface | end_effector_tool, sample_canister_seal | OSIRIS-REx TAGSAM, rover drill/scoop | F5, F12, F15 |
| instrument.atmospheric_sensor_suite | in-situ pressure/temperature/wind sensor suite | measurement range, sample rate | boom/mast mount, data, power | pressure_transducer, thermocouple_sensor, anemometer_element | Mars InSight APSS, Viking meteorology | F12, F2 |

### Docking

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| docking.port_androgynous | androgynous peripheral capture mechanism | capture envelope, ring diameter, alignment guides, standard | docking interface, structural mount, pressurized tunnel | capture_latch, alignment_guide, seal_ring, umbilical_connector | NASA Docking System (IDSS), APAS-95 | F1, F5 |
| docking.port_probe_drogue | probe-and-drogue capture mechanism | probe length, drogue cone geometry, capture tolerance | docking interface, structural mount | capture_latch, seal_ring | Soyuz/Progress probe-drogue | F1, F5 |
| docking.berthing_mechanism | robotically-berthed mating interface (not flown in) | ring diameter, bolt pattern, alignment tolerance | berthing interface, structural mount | bolt_actuator, seal_ring, umbilical_connector | Common Berthing Mechanism (ISS) | F1, F5, F15 |

### Landing

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| landing.leg_fixed | fixed (non-deploying) landing leg/skid | stance width, ground clearance, load rating | structural mount, ground pad | contact_sensor | lunar module strut (post-deploy), small lander legs | F5, F1 |
| landing.skid_runner | ground-contact runner/skid for horizontal landing | length, material, suspension | structural mount | shock_strut, brake_unit | Shuttle orbiter gear concept, Dream Chaser gear | F5, F1 |
| landing.wheel_gear | wheeled landing gear (spaceplane/aircraft-like) | wheel diameter, tire type, strut stroke | structural mount, retraction mechanism | shock_strut, brake_unit, tire_pneumatic | Shuttle orbiter main/nose gear | F5 |
| landing.rover_wheel | wheel for planetary surface mobility | diameter, width, tread pattern, material | axle/hub mount | motor_wheel_hub, suspension_strut | Curiosity/Perseverance wheels, Lunokhod | F5, F12, F6 |
| landing.suspension_rocker_bogie | rocker-bogie or similar passive suspension linkage | linkage geometry, travel, load distribution | chassis mount, wheel mounts (xN) | pivot_bearing, differential_linkage | Mars rover rocker-bogie suspension | F5, F12 |
| landing.anchoring_harpoon_foot | surface anchoring device for low-gravity bodies | penetration force, hold force, deployment type | structural mount, ground interface | harpoon_pyro, tether_winch | Philae harpoons/ice screws | F5, F12 |
| landing.terminal_descent_mount | structure carrying small throttleable descent engines plus legs | engine mount pattern, leg attach points | engine mounts (xN), leg mounts (xN) | load_cell, gimbal_bearing_ring | lunar module descent stage, Mars lander descent stage | F5, F1 |

### Ground support interfaces

These are vehicle-side mount points for GSE, not GSE itself (GSE stays on the ground / pad). See section 4 for the finding that this family does not map to any science field.

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| gse.umbilical_connector_plate | vehicle-side disconnect plate mating to ground umbilicals | connector count (power/data/fluid), release type | vehicle structural mount, ground umbilical mate | quick_disconnect_fluid, quick_disconnect_electrical, release_actuator | Falcon 9 T-0 umbilicals, Saturn V swing-arm interfaces | none (F5 only incidentally) |
| gse.fill_drain_panel | ground fluid servicing panel on the vehicle | line count, connector standard, purge provisions | ground fluid line mate, internal routing | quick_disconnect_fluid, purge_valve | propellant loading panels on most LVs | none |
| gse.ground_power_interface | vehicle-side ground power connector/relay | voltage/current rating, connector type | ground power mate, internal power bus | relay_power_switch, quick_disconnect_electrical | pre-launch ground power umbilicals | none |
| gse.transporter_erector_interface | vehicle hard points for transporter/erector/launch mount | hard-point pattern, load rating | TE mating points, hold-down mount | hold_down_release, load_cell | Falcon 9 TE hard points, Soyuz launch mount clamps | F5 only |
| gse.access_arm_interface | hard points/door for crew access arm or white room | door envelope, seal type | access arm mate, hatch | seal_ring, hatch_mechanism | Crew Access Arm interface | F14 (crew safety) only |

### Advanced placeholders (NTR, NEP/SEP, fusion, sails)

Added only to test that the taxonomy extends without a new family. See section 4 for the verdict per class.

| id | what | key params | mounts/ports | component slots | examples | fields |
|---|---|---|---|---|---|---|
| engine.nuclear_thermal_rocket | fission reactor heats propellant (LH2) through a nozzle | reactor thermal power, core material limit temp, nozzle geometry, Isp | propellant feed, thrust mount, control/instrumentation | reactor_core_fission, control_rod_drive, radiation_shield_mass, turbopump_assembly | NERVA (historical ground test), DRACO (in development) | F10, F3, F5 |
| power.fusion_reactor_core | fusion power/propulsion core, magnetic or inertial confinement | confinement type, fuel (D-T/D-He3), field strength, thermal output | power/thrust interface, coolant/shield interface, mount | magnet_coil_hts, fuel_pellet_injector, first_wall_panel, neutron_shield | ITER/NIF as physics basis; no flown spacecraft example | F9, F10, F11, F5, F3 |
| power.sail_photon_solar | large reflective membrane propelled by photon pressure | deployed area, areal density, reflectivity, boom count | boom roots (mechanism.deployable_boom parts), bus mount | membrane_reflective_film, tensioning_cable | IKAROS, LightSail 2 | F8, F5, F1 |
| power.sail_magnetic_plasma | large superconducting loop deflecting the solar wind for thrust | loop radius, current/field strength, deployment type | loop deployment root, bus mount | magnet_coil_hts | concept studies (MagSail, M2P2) | F9, F7, F5 |
| propulsion.sep_thruster_cluster_mount | structural mount for a gimballed cluster of SEP thrusters | thruster count, gimbal range, thermal interface | thruster mounts (xN), power bus | gimbal_actuator, power_processing_unit | proposed high-power SEP cargo tugs (concept) | F9, F7, F1 |

**Count: 102 part types** across the 16 named families plus the advanced-placeholder group, inside the 60-120 target.

## 2. Component types

Columns: id, what it is, key parameters, part types that typically host it, the system graph(s) it sits on (power / fluid / data / thermal / structural, scope-2026-11.md 3.2), 1-2 real examples. Grouped by primary graph; many sit on two or three.

### Fluid graph

| id | what | key params | hosting parts | graph(s) | examples |
|---|---|---|---|---|---|
| valve.ball | on/off or throttling ball valve | Cv, actuation type, seat material | tank, engine, feed lines | fluid | Falcon 9 prevalves |
| valve.poppet | fast-acting poppet valve | response time, seat leak rate | engine, RCS thruster | fluid | engine main valves |
| valve.check | one-way flow valve | cracking pressure, reverse leak rate | feed/pressurant lines | fluid | feed check valves |
| valve.relief_pressure | opens above set pressure to protect a vessel | set pressure, reseat pressure, flow capacity | tank, COPV | fluid | tank relief valves |
| valve.pyro_normally_closed | one-shot pyro valve, opens once | seal burst pressure, actuation energy | engine/pressurant lines | fluid, structural | Shuttle pyro valves |
| valve.pyro_normally_open | one-shot pyro valve, closes once (guillotine) | cut force, seal retention | safety isolation lines | fluid, structural | pyro guillotine valves |
| valve.solenoid | electrically actuated on/off valve | response time, coil power, leak rate | RCS thrusters, small lines | fluid, power, data | RCS solenoid valves |
| valve.cryogenic_shutoff | shutoff valve rated for cryo service | cryo seal material, thermal gradient tolerance | cryo tank, cryo feed line | fluid, thermal | LH2/LOX shutoffs |
| valve.burst_disk | passive overpressure relief, bursts once | burst pressure tolerance | COPV, pressurant tank | fluid, structural | COPV burst disks |
| regulator.pressure | reduces/holds downstream pressure | set pressure, flow capacity, droop | pressurant feed system | fluid | He pressure regulators |
| orifice.flow_restrictor | fixed flow restriction | diameter, discharge coefficient | feed lines, injector | fluid | flow trim orifices |
| filter.inline_mesh | traps particulates from flow | mesh rating, pressure drop, dirt capacity | feed lines, pump inlet | fluid | propellant filters |
| line.tube_rigid | rigid routed tube segment | diameter, wall thickness, bend radius, material | any part with internal routing | fluid, structural | propellant lines |
| line.flex_hose | flexible hose/bellows crossing a joint | diameter, flex life, burst pressure | gimballed engine feed, umbilicals | fluid, structural | gimbal flex lines |
| line.bellows_compensator | absorbs thermal expansion/motion in a line | stroke, spring rate, cycle life | cryo lines, gimbal interfaces | fluid, structural, thermal | cryo line bellows |
| coupling.quick_disconnect_fluid | mates/demates a fluid line without tools | flow capacity, mate/demate force, leak rate | umbilicals, docking ports | fluid | fueling QDs |
| accumulator.hydraulic | stores hydraulic fluid under pressure | volume, precharge pressure | TVC hydraulic system | fluid, structural | engine TVC accumulators |
| pump.centrifugal | rotodynamic pump for fluid transfer | flow rate, head rise, efficiency | transfer systems, coolant loops | fluid, power | transfer pumps |
| pump.turbopump_assembly | turbine-driven pump feeding an engine | flow rate, head rise, turbine power, shaft speed | liquid bipropellant engine | fluid, structural, thermal | Merlin/Rutherford turbopumps |
| injector.element_pattern | meters/atomizes propellant into the chamber | element count/pattern, pressure drop | liquid/hybrid engine | fluid, structural | engine injector plates |
| igniter.pyrotechnic | one-shot pyrotechnic igniter | energy output, burn time | solid motor, engine startup | fluid, structural | SRM igniters |
| igniter.torch | repeatable spark/torch igniter | spark energy, duty cycle | restartable liquid engines | fluid, power | spark-torch igniters |
| igniter.hypergolic_slug | hypergolic fluid slug for ignition | slug volume, response time | hypergolic engines | fluid | Draco/SuperDraco ignition |
| catalyst_bed | decomposes monopropellant exothermically | bed material, preheat temp, pressure drop | monoprop engine/thruster | fluid, thermal | Shell 405 catalyst beds |
| heat_exchanger.plate | compact plate heat exchanger | plate count, area, approach temperature | power-conversion/thermal loops | fluid, thermal | Brayton-cycle HX |
| heat_exchanger.shell_tube | shell-and-tube heat exchanger | tube count, area, pressure rating | reactor coolant loop | fluid, thermal | reactor loop HX |
| diaphragm.bladder | flexible membrane separating pressurant from propellant | material, cycle life, expulsion efficiency | storable propellant tank | fluid, structural | bladder tanks |
| screen.capillary_pmd | surface-tension device positioning propellant in low-g | mesh geometry, bubble point | propellant tank | fluid | propellant management devices |
| baffle.antislosh | internal baffle damping propellant slosh | ring count, open-area ratio | propellant tank | fluid, structural | anti-slosh baffles |
| pressure_transducer | measures fluid pressure onto the data bus | range, accuracy, response time | tank, feed line, chamber | fluid, data | tank pressure sensors |
| level_sensor_cryo | measures cryogenic propellant level | sensing method, accuracy | cryo tank | fluid, data | LOX/LH2 level sensors |
| gas_generator_inflator | pyro gas source inflating an airbag/HIAD | gas yield, burn time | airbag system, HIAD | fluid, structural | Pathfinder airbag inflators |
| inflation_gas_bottle | stored-gas source for inflatables | volume, pressure, gas type | HIAD, ballute | fluid | HIAD inflation bottles |
| purge_valve | vents/purges a line or cavity with inert gas | flow capacity, actuation | fuel/ox interfaces | fluid | purge system valves |
| torus_bladder | inflatable torus membrane forming an aeroshell ring | material, diameter, pressure rating | inflatable decelerator | fluid, structural | HIAD torus |
| tps_flexible_blanket | flexible TPS fabric layer | material layup, temperature rating | HIAD, flexible TPS | thermal, structural | HIAD TPS layer |

### Thermal graph

| id | what | key params | hosting parts | graph(s) | examples |
|---|---|---|---|---|---|
| heat_pipe.fixed_conductance | passive two-phase heat transport device | working fluid, wick structure, length | radiator panel, coldbox | thermal | constant-conductance heat pipes |
| heat_pipe.variable_conductance_lhp | loop heat pipe, variable conductance | reservoir control temp, transport distance | deployable radiator | thermal | LHPs on sat radiators |
| mli_blanket.multilayer | multi-layer insulation blanket | layer count/material, seam coverage | cryo tank, coldbox, habitat shell | thermal | cryo tank MLI |
| heater.resistive_patch | resistive electrical heater element | power, area, setpoint | tank, line, battery | thermal, power, data | line/battery heaters |
| thermostat.bimetallic_or_solid_state | switches a heater/valve at a setpoint | setpoint, hysteresis | heater circuits | thermal, data | thermostatic heater control |
| cryocooler.stirling | active mechanical cooler, Stirling cycle | cooling power, input power, vibration output | coldbox, cryo instrument | thermal, power, structural | JWST MIRI cryocooler |
| cryocooler.pulse_tube | active cooler, pulse-tube cycle | cooling power, input power | coldbox | thermal, power | pulse-tube coolers |
| radiator_coating.optical_surface | thermal-control coating/OSR on a radiator | emissivity, solar absorptivity, degradation rate | radiator panel | thermal | OSR tiles, white paint |
| thermal_strap.flexible_copper | flexible conductive strap across a joint | conductance, flex life | instrument-to-radiator path | thermal, structural | copper braid straps |
| phase_change_material_pack | latent-heat thermal buffer | PCM material, mass, melt temperature | equipment bay, EVA suit | thermal | PCM thermal buffers |
| louver.thermal | mechanical louver modulating radiator area | blade count, actuation range | radiator panel | thermal, structural | bimetallic louvers (Voyager) |
| coldplate.component_mount | cold plate conducting component heat into a loop | area, channel layout | equipment bay | thermal, structural | electronics coldplates |
| insulation.foam_rigid | rigid foam insulation layer | density, thickness, material | cryo tank exterior | thermal | Shuttle ET foam |
| insulation.aerogel | very low-conductivity solid insulation | density, thickness | instrument, cryo line | thermal | aerogel insulation blocks |
| ablator_tile.charring | ablative TPS material unit | material formulation, thickness, char rate | heatshield_ablative | thermal, structural | PICA tiles |
| tps_tile.ceramic_reusable | reusable ceramic tile for TPS | material, thickness, strain isolation | heatshield_reusable_tps | thermal, structural | Shuttle LI-900 tiles |
| strain_isolation_pad | compliant pad between a tile and the structure | material, thickness | reusable TPS | structural, thermal | Shuttle SIP |
| gap_filler.tps | flexible filler between TPS tiles | material, compressibility | reusable TPS | thermal | Shuttle gap fillers |
| vacuum_getter_pump | maintains vacuum in an insulation annulus | pumping capacity, getter material | cryo_vacuum_jacketed tank | thermal | vacuum-jacket getters |
| radioisotope_heat_unit | small radioisotope unit for spot/survival heating | isotope loading, heat output | rover/lander electronics bay | thermal, F10 | RHUs on Mars rovers |

### Power graph

| id | what | key params | hosting parts | graph(s) | examples |
|---|---|---|---|---|---|
| battery.cell_pack | group of electrochemical cells wired together | chemistry, cell count/arrangement, capacity, C-rate | battery_module_housing, equipment_bay | power, thermal | Li-ion packs |
| battery.management_unit | monitors/protects/balances a battery pack | channel count, protection thresholds | battery_module_housing | power, data | BMS boards |
| solar_cell.photovoltaic_multijunction | PV cell laminated to an array panel | junction count, efficiency, mass per area | solar_array_rigid/flexible | power, F7 | triple-junction GaAs cells |
| bypass_diode | protects a solar string from a shaded/failed cell | forward voltage, current rating | solar array panel | power | array bypass diodes |
| blocking_diode | prevents reverse current into a string/source | voltage/current rating | solar array, battery bus | power | array blocking diodes |
| dc_dc_converter | converts bus voltage between levels | input/output voltage, power, efficiency | equipment bay, fuel cell module | power | point-of-load converters |
| power_conditioning_unit | conditions/distributes raw power to a bus standard | input source, output regulation, fault handling | equipment bay, electric thruster mount | power, data | spacecraft PCUs |
| charge_controller_mppt | tracks max power point charging a battery from an array | tracking algorithm, voltage range | battery_module_housing | power, data | MPPT charge controllers |
| bus_regulator_shunt | dissipates excess array power to hold bus voltage | shunt stages, dissipation capacity | equipment bay | power | sequential shunt units |
| slip_ring.rotary | transfers power/data across a rotating joint | channel count, current rating, life | rotating array yoke, centrifuge habitat | power, data, structural | array drive slip rings |
| fuse.power | one-shot overcurrent protection | rated current, breaking capacity | power distribution unit | power | line fuses |
| relay.power_switch | electromechanical/solid-state power switch | current rating, switching life | power distribution unit | power, data | load switches |
| current_sensor | measures current on a power line | range, accuracy | power distribution unit | power, data | shunt/Hall current sensors |
| wiring_harness.power_bundle | routed bundle of power conductors | gauge, length, bundle diameter | any part with internal routing | power, structural | power harnesses |
| fuel_cell.pem_stack | stack converting H2+O2 to electricity and water | cell count, active area, power rating | fuel_cell_module | power, F4 | Shuttle/Gemini fuel cell stacks |
| thermoelectric_module | converts heat to electricity via Seebeck effect | couple count, hot/cold side temps | RTG | power, F7 | RTG thermoelectric couples |
| radioisotope_heat_source | sealed radioisotope fuel module (GPHS) | isotope, loading, decay curve | RTG, NTR | power, F10 | GPHS modules |
| reactor_core.fission | fission fuel/moderator assembly | fuel form, enrichment, core geometry, thermal power | fission_reactor_surface, NTR | power, thermal, F10 | Kilopower core |
| control_rod_drive | moves control rods/reflectors to regulate reactivity | stroke, drive force, response time | fission_reactor_surface | power, structural, F10 | reactor control drives |
| coolant_pump.reactor_loop | circulates reactor coolant | flow rate, head, radiation tolerance | fission_reactor_surface | fluid, power | reactor loop pumps |
| generator.brayton_turboalternator | closed Brayton-cycle turbine/alternator | turbine inlet temp, pressure ratio, output power | fission_reactor_surface | power, thermal, fluid | Brayton conversion units |
| generator.stirling_convertor | Stirling-cycle heat-to-power convertor | hot/cold end temps, stroke, output power | RTG alternative | power, thermal | ASRG (developed, not flown) |
| power_processing_unit | converts/regulates power for an electric thruster | input voltage, output power, efficiency | electric thrusters | power, data | Hall/ion thruster PPUs |
| magnet_coil.electromagnet | conventional wound electromagnet | turns, current, field strength | magnetic_torquer, Hall thruster | power, F8 | torque rod coils |
| magnet_coil.hts_superconducting | high-temperature superconducting coil | critical current, field strength, cryo interface | MPD/VASIMR, fusion core, magsail | power, thermal, F7 | lab HTS magnets |

### Data graph

| id | what | key params | hosting parts | graph(s) | examples |
|---|---|---|---|---|---|
| flight_computer_card | processing card inside a flight computer | clock speed, rad-hardness, memory | flight_computer_unit, subsystem_controller | data, power | RAD750-class cards |
| memory_module_radhard | radiation-hardened memory module | capacity, error correction | flight computer, data handling unit | data | rad-hard SDRAM modules |
| solid_state_recorder | mass data storage recorder | capacity, read/write rate | data_handling_unit | data, power | instrument data recorders |
| fpga_card.processing | reconfigurable processing card | gate count, I/O count | data handling unit, subsystem controller | data, power | instrument FPGA boards |
| data_bus_node | interface node on a vehicle data bus | protocol (1553/SpaceWire/CAN), address | any avionics-hosting part | data | 1553 remote terminals |
| adc_card | analog-to-digital conversion card | channel count, resolution, sample rate | sensor_interface_unit | data | instrumentation ADC cards |
| signal_conditioning_card | amplifies/filters sensor signals before digitizing | gain range, filter bandwidth | sensor_interface_unit | data | strain/thermocouple conditioners |
| rf_frontend_card | RF receive/transmit front end | frequency band, noise figure, power | transponder, gnss_receiver | data, power | RF front-end boards |
| software_defined_radio_card | programmable RF modem | waveform flexibility, data rate | transponder, laser terminal, relay unit | data, power | SDR cards |
| processing_card.instrument | dedicated processing for an instrument's raw data | throughput, interfaces | imaging/spectrometer instruments | data, power | instrument DPU cards |
| watchdog_timer | resets a computer on hang detection | timeout window | flight computer unit | data | computer watchdogs |
| encoder_position | measures a mechanism's position | resolution, interface | robotic arm, gimbal mount, hinge | data, structural | joint encoders |
| tachometer | measures rotational speed | range, accuracy | reaction wheel, turbopump | data | wheel/turbopump tachometers |
| dosimeter | measures accumulated radiation dose | sensitivity, range | radiation shelter, particle detector, EVA suit | data, F14 | crew dosimeters |

### Structural graph

| id | what | key params | hosting parts | graph(s) | examples |
|---|---|---|---|---|---|
| fastener.bolted_joint | threaded mechanical fastener joint | size, preload, material | any two mating structural parts | structural | standard aerospace bolts |
| separation_bolt.pyrotechnic | bolt with pyro-severed shank | preload, sever energy | separation_system, payload_adapter | structural | stage separation bolts |
| clamp_band.marman | circumferential V-band clamp joint | tension, segment count | clamp_band_release, payload_adapter | structural | Marman clamp bands |
| frangible_nut | nut that fractures on command to release a bolt | preload, fracture energy | separation_system | structural | frangible nut separation |
| pyro_cutter.cable | pyro device severing a cable/strap | cable size, cut energy | fairing separation, chute riser cutters | structural | cable cutters |
| bolt_cutter.pyro | pyro device severing a bolt or band | bolt size, cut energy | clamp_band_release | structural | Marman band bolt cutters |
| bracket.mounting | structural mounting bracket for a component | load rating, material | equipment_bay, any component-hosting part | structural | component mounting brackets |
| damper.vibration_isolator | isolates a component from structural vibration | stiffness, damping ratio | equipment_bay, instrument mounts | structural | wire-rope/elastomer isolators |
| spring.preload_or_release | mechanical spring storing/releasing energy | spring rate, preload, stroke | separation_system, deployment_hinge | structural | separation push-off springs |
| torsion_spring | rotational spring driving a hinge | torque curve, windup angle | deployment_hinge | structural | solar panel hinge springs |
| latch_mechanism | mechanical latch holding a deployed/mated state | holding force, release trigger | deployment_hinge, docking_latch, landing legs | structural, data | deployment latches |
| position_sensor | detects a mechanism's end position (deployed/stowed) | type (microswitch/Hall), accuracy | deployment_hinge, landing legs, valves | data, structural | deployment limit switches |
| drive_motor.mechanism | electric motor driving a deployment/rotation mechanism | torque, speed, power | deployment_hinge, deployable_boom, arm joint | power, structural | mechanism drive motors |
| joint.hinge_pin | simple mechanical rotational joint pin | diameter, material, bearing surface | deployment_hinge, suspension | structural | hinge pins |
| bearing.rotary_assembly | rolling-element bearing for a rotating joint | load rating, life, lubrication | reaction_wheel, slip_ring, rotary structures | structural | wheel/gimbal bearings |
| motor_winding | electromagnetic winding driving a rotary actuator | turns, resistance, torque constant | reaction_wheel, torquer, drive motors | power, structural | BLDC motor windings |
| gyroscope_element | inertial rate-sensing element | type (MEMS/RLG/FOG), bias, scale factor | imu_inertial | data, structural | gyro elements |
| accelerometer_element | inertial acceleration-sensing element | range, bias, scale factor | imu_inertial | data, structural | accelerometer elements |
| crush_core.honeycomb | sacrificial energy-absorbing core | crush strength, stroke | landing legs, seat couch | structural | aluminum honeycomb crush cores |
| load_limiter_strut | strut yielding at a set load to limit transmitted force | trigger load, stroke | seat_couch, landing legs | structural | crew seat load limiters |
| contact_sensor | detects ground/surface contact | trigger threshold, response time | landing legs, anchoring foot | data | landing contact switches |
| shock_strut | oleo/spring-damper landing gear strut | stroke, spring rate, damping | skid_runner, wheel_gear | structural | oleo-pneumatic struts |
| brake_unit | friction brake on a wheel | torque capacity, actuation | wheel_gear | structural, power | wheel brakes |
| tire_pneumatic | inflatable/composite tire | diameter, load rating, material | wheel_gear, rover wheel | structural | aircraft-style tires |
| motor_wheel_hub | in-wheel drive motor and hub | torque, speed, reduction ratio | rover_wheel | power, structural | rover wheel motors |
| suspension_strut | passive/active suspension member | stiffness, damping, travel | rover chassis, suspension linkage | structural | rover suspension struts |
| pivot_bearing | bearing at a suspension pivot | load rating, life | suspension_rocker_bogie | structural | rocker-bogie pivots |
| differential_linkage | mechanical linkage balancing load across a suspension | geometry, load split ratio | suspension_rocker_bogie | structural | rocker-bogie differential |
| harpoon_pyro | pyro-fired anchoring harpoon | penetration energy, barb geometry | anchoring_harpoon_foot | structural | Philae harpoons |
| tether_winch | winch managing a tether's length/tension | spool capacity, tension control | anchoring foot, tether systems | structural, data | anchoring tethers |
| end_effector_tool | gripping/sampling tool at an arm's end | grip force, tool type | robotic_arm, sample_collection_arm | structural, data | TAGSAM head, grapple fixtures |
| joint_actuator.robotic | actuator at a manipulator joint | torque, range, backlash | robotic_arm | power, structural, data | arm joint actuators |
| hold_down_release | releases a vehicle from its launch mount | holding force, release time | transporter_erector_interface | structural | hold-down release mechanisms |
| load_cell | measures force/load at a structural interface | range, accuracy | thrust_structure, terminal_descent_mount | data, structural | engine thrust load cells |
| gimbal_bearing_ring | bearing ring letting an engine/mount pivot | range of motion, load rating | thrust_structure, terminal_descent_mount | structural | engine gimbal bearings |
| hydraulic_actuator | hydraulic linear/rotary actuator | stroke/torque, pressure, response rate | gimbal_actuator_mount | fluid, structural, power | TVC hydraulic actuators |
| electromechanical_actuator | motor-driven linear/rotary actuator | stroke/torque, speed, power | gimbal_actuator_mount, robotic joints | power, structural, data | EMA gimbal actuators |
| insert.potted | load-carrying insert bonded into a sandwich panel | pull-out strength, material | panel_sandwich | structural | honeycomb panel inserts |
| doubler.reinforcement | local reinforcement patch at a stress concentration | material, thickness, extent | panel_sandwich, ring_frame | structural | panel doublers |

### Instrument and sensor components

| id | what | key params | hosting parts | graph(s) | examples |
|---|---|---|---|---|---|
| optical_detector_array | imaging sensor array (CCD/CMOS/microbolometer) | pixel format, spectral response, readout rate | imaging_camera, star_tracker, horizon_sensor | data, power | CCD/CMOS focal planes |
| filter_wheel | selectable optical filter mechanism | filter count, switch time | imaging_camera | structural, data | camera filter wheels |
| focal_plane_cooler | cools a detector to reduce noise | cooling power, target temperature | imaging_camera, spectrometer | thermal, power | IR focal plane coolers |
| diffraction_grating | disperses light by wavelength | groove density, blaze angle | spectrometer | F8, structural | spectrometer gratings |
| calibration_lamp | onboard light source for instrument calibration | spectral output, life | spectrometer | power, data | calibration lamps |
| ion_source | ionizes sample material for a mass spectrometer | ionization method, efficiency | mass_spectrometer | power, fluid | EI/CI ion sources |
| mass_analyzer | separates ions by mass-to-charge ratio | analyzer type, resolution | mass_spectrometer | structural, power | quadrupole analyzers |
| electron_multiplier_detector | detects individual ions/electrons | gain, dead time | mass_spectrometer, particle_detector | data, power | channel electron multipliers |
| fluxgate_sensor_element | measures magnetic field via the fluxgate effect | sensitivity, bandwidth | magnetometer | data, power | fluxgate sensors |
| solid_state_detector | semiconductor radiation detector | energy resolution, active area | particle_detector | data, power | silicon particle detectors |
| scintillator_element | converts radiation to light for detection | material, light yield | particle_detector | F10, data | scintillator crystals |
| photomultiplier_tube | amplifies scintillator light to an electrical signal | gain, quantum efficiency | particle_detector | power, data | PMTs |
| antenna_dipole_deployable | deployable dipole/monopole radar antenna element | stowed/deployed length, frequency | radar_sounder | structural, F8 | MARSIS dipole antennas |
| sample_canister_seal | hermetic seal on a returned-sample canister | seal material, leak rate | sample_collection_arm | structural | OSIRIS-REx sample capsule seal |
| anemometer_element | measures local wind speed/direction | range, response time | atmospheric_sensor_suite | data, power | lander wind sensors |

### Crew and life-support components

| id | what | key params | hosting parts | graph(s) | examples |
|---|---|---|---|---|---|
| co2_scrubber | removes CO2 from cabin air | sorbent type, capacity, regeneration method | life_support_rack | fluid, power, F4 | ISS CDRA |
| o2_generator | generates oxygen (electrolysis or stored) | production rate, power draw | life_support_rack | fluid, power, F4 | ISS OGS |
| humidity_condenser | removes humidity from cabin air | condensing capacity | life_support_rack | fluid, thermal | condensing heat exchangers |
| water_recovery_unit | reclaims potable water from waste streams | recovery rate, purity | life_support_rack | fluid, F4 | ISS WRS |
| pressure_relief_valve.habitat | protects a crewed shell from overpressure | set pressure, flow capacity | pressurized_module_shell | fluid, structural | cabin relief valves |
| window_pane.pressure | transparent pressure-rated viewport | material, thickness, area | pressurized_module_shell | structural, F8 | Cupola windows |
| hatch_mechanism | operable pressure hatch | seal type, actuation, interlock | pressurized_module_shell, airlock_module | structural, data | docking/EVA hatches |
| depress_valve | controlled venting valve for airlock depressurization | flow rate, control precision | airlock_module | fluid | airlock depress valves |
| suit_umbilical_port | supplies power/O2/cooling to a suited crew member | connector standard, flow/power capacity | airlock_module, pressurized shell | fluid, power, data | EMU umbilical ports |
| portable_life_support_pack | self-contained suit life support | O2 supply, CO2 removal, duration | eva_suit | fluid, thermal, F14 | PLSS backpack |
| restraint_harness | crew restraint webbing/buckle set | load rating, adjustment range | seat_couch | structural, F14 | 5/7-point harnesses |
| radiation_shield_mass | mass placed specifically to attenuate radiation | material, areal density | radiation_shelter, instrument shielding | structural, F10, F11 | polyethylene/water shielding |

### Docking, GSE and misc. components

| id | what | key params | hosting parts | graph(s) | examples |
|---|---|---|---|---|---|
| capture_latch | mechanical latch capturing a docking/berthing interface | capture force, release trigger | docking ports, berthing mechanism | structural, data | NDS capture latches |
| alignment_guide | passive guide aligning two mating interfaces | geometry, tolerance | docking ports | structural | docking alignment petals |
| seal_ring | pressure/vacuum seal at a mating interface | material, compression set | docking ports, hatches, access arm | fluid, structural | docking interface seals |
| umbilical_connector | transfers power/data/fluid across a mated interface | channel mix, mate force | docking ports, berthing mechanism | power, data, fluid | docking umbilical connectors |
| bolt_actuator | powered bolt driving a berthing mechanism closed | torque, preload achieved | berthing_mechanism | structural, power | CBM bolt actuators |
| quick_disconnect_electrical | mates/demates electrical lines without tools | channel count, mate/demate cycles | GSE interfaces, docking umbilicals | power, data | ground power QDs |
| release_actuator | releases an umbilical plate from the vehicle at a trigger | release force, timing accuracy | umbilical_connector_plate | structural | umbilical release mechanisms |

### Advanced-placeholder components

| id | what | key params | hosting parts | graph(s) | examples |
|---|---|---|---|---|---|
| fuel_pellet_injector | injects fusion fuel pellets into a confinement region | pellet rate, velocity | fusion_reactor_core | fluid, F9 | ICF pellet injectors (lab) |
| first_wall_panel | plasma-facing panel absorbing reactor heat flux | material, cooling channel layout, heat flux rating | fusion_reactor_core | thermal, structural, F9 | tokamak first-wall concepts |
| neutron_shield | shielding mass attenuating fusion/fission neutron flux | material, thickness | fusion_reactor_core, fission_reactor_surface | structural, F10 | lithium/boron shielding |
| membrane_reflective_film | ultra-thin reflective membrane material | areal density, reflectivity, tear resistance | sail_photon_solar | structural (flagged, sec.4) | Kapton/CP1 sail film |
| tensioning_cable | cable maintaining tension across a deployed membrane/boom | tension, material, length | sail_photon_solar, solar_array_flexible | structural | sail/array tensioning cables |

**Count: 170 component types** across the five system graphs, inside the 100-200 target.

## 3. Boundary cases: part or component?

The owner's rule of thumb (3.1, 5.2) is "if a real program buys it from a supplier or swaps it between production runs, it's a component." Taken alone this would make engines components, since engines are bought/supplier items too — but 3.1 explicitly makes the whole Rutherford a part ("Rutherford x9, each Rutherford one part") while its pumps/valves/igniter are its components. The rule that actually survives contact with the owner's own example is narrower: **a thing is a part if it needs its own rigid-body/physics state and its own exterior, placed at a mount face; it is a component if it only adds mass and drives its host's internal logic.** The supplier/swap test still matters, but as a tiebreaker, not the primary test. Applied below:

| Item | Recommendation | Why |
|---|---|---|
| Reaction wheel | **Part** | Its spinning rotor is a real angular-momentum store with its own gyroscopic physics that the craft's attitude dynamics must see directly; it mounts at its own mount face, same pattern as an engine. Its windings/bearings/tachometer are its components. |
| Battery pack | **Component** | No separate exterior or rigid-body behaviour the craft needs; it only adds mass and feeds the power graph. 3.1 itself lists `battery.cell_pack` in the same breath as `valve.ball` and `line.tube` as a typed, parameterized item, not alongside `tank.cylindrical` as a part (line 41). It is hosted inside a `power.battery_module_housing` or `structure.equipment_bay` part. |
| RCS quad/pod | **Split: each thruster is a part, the bracket is a part, the "quad" is a group** | Each thruster has its own small rigid body and nozzle exterior, same logic as "each Rutherford is one part." A shared mounting bracket is its own small structure part exposing 4 mount faces. "Quad" is a 3.1 *group*: editor/staging label only, no physics, so engine count can change (composability test, line 45) without new code. |
| Solar array wing | **Split: each rigid/semi-rigid panel segment is a part, the deployment boom/hinge is a part, the cells are components** | A wing is really several parts joined at ports (panel segments, yoke, boom, hinge), consistent with "joining parts joins their ports" (3.1 line 34). The PV cells, diodes, and harness inside a panel are components. Treating the whole wing as one part would hide a real joint (the hinge) that the structural-load graph needs to resolve separately. |
| Parachute | **Part, but flagged for a two-model split (section 4)** | Packed, it is one placeable object (canister) with components (riser, pilot chute, mortar). Deployed, it is not rigid and needs a membrane/aero model unlike anything else in the catalog — see section 4, this is the clearest "split it" case in the catalog. |
| COPV | **Part** | 3.1 names the COPV directly as an example of whole-part pressure-vessel failure ("a COPV burst is one event," line 36), which only makes sense if a COPV is itself a part with its own structural-cell mesh, not a component hidden inside a tank's parameter list. |
| Engine turbopump | **Component** | 3.1 says this explicitly: "A Rutherford's e-pumps, main valves, gimbal actuators and igniter are its components" (line 31). It adds mass and drives the engine part's internal fluid/structural logic; it has no exterior of its own in ship view. |
| Docking port | **Part** | It has its own exterior, its own mount face/port definition (3.1 line 34: "ports on mount faces: fluid couplings, connectors, umbilicals"), and different standards (IDSS, APAS, probe-drogue) are swapped at the vehicle's forward interface exactly like an interstage or adapter. Its latches, seals, and alignment guides are components. |
| Fuel cell stack | **Component** | Same logic as the battery: it is a swappable supplier stack that adds mass and drives a power-module part's logic; the housing with its reactant/coolant ports is the part. |
| Avionics "black box" (flight computer, PDU, transponder) | **Part** | Each is bought as a sealed unit but is also the smallest placeable thing with its own exterior and mount face — matching the engine precedent, not the battery one. Its boards (`flight_computer_card`, `rf_frontend_card`) are components. |
| Cryocooler | **Component, hosted in a coldbox/equipment part** | It adds cooling capacity and mass to whatever part's thermal node it sits on; it rarely has an exterior that matters in ship view separate from its host box. Borderline: a very large standalone cooler mounted directly to a truss (ISS-style ORU) would push this toward "part," so flag for a size-based exception rather than a type-based one. |
| Deployable boom / antenna | **Part** | Needs its own articulated rigid-body state (stowed vs. deployed length, tip-mass dynamics) distinct from whatever it is mounted on; this is the same reasoning as the solar-array hinge case. |
| TPS tile / ablator patch | **Neither cleanly — see section 4** | A tile is too small and too numerous to be a part (thousands per heatshield) and too geometry-coupled to be a normal component (its "parameter" is really a cell-level material/thickness map on the heatshield part's structural/thermal mesh, per 3.1's own cell-vs-part distinction, line 32). Recommend modelling TPS as a material/thickness field over the heatshield part's cells, not as a component type at all. |

**Pattern that emerges:** the "is it a part" test that actually matches the owner's own two examples (engine, Rutherford) is about *needing independent rigid-body/exterior state*, not about procurement. Everything that is bought from a supplier but only adds mass + logic to a host (batteries, fuel cells, turbopumps, cryocoolers) is a component; everything that is bought from a supplier and also needs its own placement, exterior, and mount face (engines, reaction wheels, docking ports, avionics boxes, booms) is a part. The few cases that resist this split (TPS tiles, large membranes) are exactly the cases flagged in section 4.

## 4. Gaps and mesh checks

### Types needing a physics domain the 15 fields don't clearly own

- **Gossamer/membrane structures (top finding).** Parachutes (deployed), solar sails, magnetic-sail loops, HIAD/ballute inflatables, and large thin-film arrays all need a structural model that is never meaningfully rigid: large-deflection membrane dynamics, often fluid-structure-coupled (inflation, chute opening shock). Section 3.2's structural proposal is "rigid-body motion plus a load path and margin... flex modes come later, only where they matter (POGO, long stacks)." That is flex-modes-of-an-otherwise-rigid-part; it has no stated path for a part that is never rigid. This also collides with 3.1's open question 9.6 (max size per part type before cell-level failure is needed) — a solar sail or magsail loop can be kilometres across, well past any sensible single-rigid-body size. Recommend: a named "membrane/gossamer" part class with its own cell-level solver (closer to a cloth/shell FEM than a rigid-body-plus-margins model), decided in session 1 alongside the big-parts question.
- **Hypervelocity impact / MMOD.** Whipple-shield and micrometeoroid-and-orbital-debris impact physics (shock, spall, fragmentation) is distinct from both the structural margin model (3.2) and the SU2-calibrated aero/thermal model — it's a short, extreme-strain-rate event, closer to SP-8042's domain. It touches F5 (material response) and F11 (high-energy particle/fundamental physics) but neither field names it as an unlock. Recommend folding it into F5 explicitly, or noting it as a named cross-field capability rather than leaving it implicit.
- **Granular mechanics / terramechanics.** Rover wheel-soil interaction, landing-plume cratering, and regolith excavation for ISRU are a genuine discipline (soil mechanics of granular media), not solid mechanics (F5, which is about continuum materials) or fluid dynamics (F2, continuum flow) in the strict sense. Currently it would have to be smuggled into F12 (planetary science, which is about the *environment*, not the *mechanics of interacting with it*) or F5. Recommend an explicit note under F5 or F12 that terramechanics is in scope, since several named part types (`landing.rover_wheel`, `landing.suspension_rocker_bogie`, ISRU excavation) depend on it.
- **Vibro-acoustics.** Launch/fairing acoustic loads and random-vibration qualification sit across F2 (acoustic propagation) and F5 (structural response); both fields can plausibly claim it but neither names it. Low-severity gap (cross-field is expected per 5.1 principle 2) but worth a one-line mention so a node for "better acoustic/vibration environment prediction" has a home.
- **Spacecraft charging / surface plasma interaction.** ESD and surface charging from ambient plasma is adjacent to F9 (plasma physics) but F9's listed unlocks are all propulsion/confinement-flavoured (Hall/ion/MPD thrusters, magnetic nozzles, fusion confinement). Recommend a one-line addition to F9 covering environment-plasma interaction, since `power.solar_array_rigid/flexible` and any exposed high-voltage component depend on it.

### Types that don't map cleanly to any science field

- **Ground support equipment interfaces** (`gse.*` family). Nobody researches a better umbilical mast via nuclear physics or materials science in any interesting way — GSE is industrial/mechanical engineering with no node-gated readiness curve. These part types should probably sit entirely outside the four-layer science system (5.2): always available, costed in money/mass only, never field- or node-gated. Worth a one-line carve-out in 5.2 so design-session 1 doesn't try to force GSE through the node system.
- **Pure structural fasteners/brackets** (`fastener.bolted_joint`, `bracket.mounting`, `doubler.reinforcement`). These are parametrized entirely by F5's material/strength ranges; they don't get their own node ("better bolt" isn't a tech unlock), they just ride F5's S-curve. Not a true gap, but worth confirming in session 1 that "components with no node of their own, only a field-level parameter" is an accepted category — most fasteners, brackets, and dampers in the catalog are this way.
- **Ballast / inert mass simulators** used on test flights — same bucket as GSE, no science field, pure mass+cost.

### Types that would need two very different models (split candidates)

- **Parachute**: packed (small rigid-ish part, structural/fluid components) vs. deployed (large flexible membrane in an aerodynamic flow field). See boundary case above — this is the sharpest example in the catalog and the one the task brief itself flags.
- **Inflatable structures** (HIAD, inflatable habitat module, airbags): stowed (small, dense, near-rigid part) vs. inflated (large, thin-walled pressure membrane under aerodynamic or structural load). Same underlying issue as the parachute, and the same fix (a membrane solver) would cover both — recommend treating "deployable membrane" as one split-aware part behaviour rather than solving it once per type.
- **Ablative heatshield thermal response**: the shield's geometry is a fixed part shape, but its thermal behaviour is a *receding boundary* (material is lost as it chars/ablates), which is a fundamentally different problem from the lumped thermal node network described in 3.2 for everything else. This doesn't split the part in two, but it does mean the thermal graph's "node/link network" solver needs an explicit ablation extension, not just finer cells.
- **Nuclear thermal rocket**: checked against the catalog's engine pattern (3.1's "chamber/nozzle is defined geometry, what happens inside comes from a table") and it extends cleanly — the reactor core becomes a component feeding the same kind of performance table a chemical engine's combustion does. No split needed; this is a taxonomy success, not a gap.
- **NEP/SEP**: extends cleanly with existing part types (large deployable radiator + large deployable array + electric thruster cluster mount); no new category needed beyond what's already in the catalog. Taxonomy success.
- **Fusion reactor core**: extends with existing component patterns (magnet coils, shielding, heat exchangers) plus new but structurally unremarkable components (`fuel_pellet_injector`, `first_wall_panel`); the hard part is the physics inside the core (F9/F10/F11), not the part/component split. Taxonomy success at the type level.
- **Sails (photon and magnetic)**: extend cleanly as part types, but both lean on the same unresolved membrane/large-loop structural model flagged above as the top gap. Not a taxonomy failure, but a shared dependency worth resolving once rather than per-sail-type.

### Mesh checks that resolve cleanly (included for completeness, not gaps)

- **Life-support consumables** (O2, CO2, water) don't need a sixth system graph — they ride the existing fluid graph as ordinary nodes, with crew metabolism as a source/sink term on that graph. No new graph needed.
- **Propellant slosh coupling to attitude control** (tank → structural/fluid → astrodynamics/control) is expected cross-field behaviour (5.1 principle 2) and is exactly the kind of coupling SP-8009 (slosh) and SP-8049 (POGO) already model in the real world; the catalog's `baffle.antislosh` and `tank.*` parameters are enough to carry it, the coupling lives in the solver, not in a new type.
- **Radiation shielding for crew** correctly spans F5 (material), F10/F11 (source physics), and F14 (dose tolerance) at once — this is multi-field by design, not a gap, and the catalog's `radiation_shield_mass` component is shared by `crew.radiation_shelter` and several instrument parts without needing a type of its own per field.
