package incari.features.schematics;

import static org.assertj.core.api.Assertions.assertThat;

import incari.features.schematics.format.BlockSpec;
import java.util.Map;
import org.junit.jupiter.api.Test;

class MaterialsTest {

    @Test
    void blocksCostWhatPlacingThemTakes() {
        assertThat(Materials.cost(BlockSpec.of("minecraft:stone", Map.of()))).isEqualTo(1);
        assertThat(Materials.cost(BlockSpec.of("minecraft:oak_door", Map.of("half", "upper"))))
                .isZero();
        assertThat(Materials.cost(BlockSpec.of("minecraft:oak_door", Map.of("half", "lower"))))
                .isEqualTo(1);
        assertThat(Materials.cost(BlockSpec.of("minecraft:red_bed", Map.of("part", "head"))))
                .isZero();
        assertThat(Materials.cost(BlockSpec.of("minecraft:oak_slab", Map.of("type", "double"))))
                .isEqualTo(2);
        assertThat(Materials.cost(BlockSpec.of("minecraft:candle", Map.of("candles", "3"))))
                .isEqualTo(3);
        assertThat(Materials.cost(BlockSpec.of("minecraft:snow", Map.of("layers", "5"))))
                .isEqualTo(5);
    }
}
