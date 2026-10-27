/**
 * Schematic files, read clean-room from their formats (ADR 0066): NBT, then {@code .litematic},
 * Sponge {@code .schem} and the game's structure {@code .nbt}, into one {@link
 * incari.features.schematics.format.Schematic}. No game types: block states stay as their names.
 */
@NullMarked
package incari.features.schematics.format;

import org.jspecify.annotations.NullMarked;
