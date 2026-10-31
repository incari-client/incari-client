package incari.features.customsky;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.within;

import org.junit.jupiter.api.Test;

class SkyLayerTest {

    private static final String FILE = "optifine/sky/world0/sky1.properties";

    @Test
    void aBareFileIsItsOwnTextureAlwaysShownAdded() {
        SkyLayer layer = SkyLayer.parse("", FILE);
        assertThat(layer).isNotNull();
        assertThat(layer.source()).isEqualTo("minecraft:optifine/sky/world0/sky1.png");
        assertThat(layer.blend()).isEqualTo(SkyLayer.Blend.ADD);
        assertThat(layer.brightness(18000, 0, 0)).isEqualTo(1);
        assertThat(layer.angle(1.5F)).isEqualTo(1.5F);
        assertThat(layer.axisZ()).isEqualTo(1);
    }

    @Test
    void readsOptiFinesKeys() {
        SkyLayer layer = SkyLayer.parse("""
                # a night sky
                source=./stars.png
                blend=alpha
                rotate=true
                speed=2
                axis=0.0 2.0 0.0
                """, FILE);
        assertThat(layer).isNotNull();
        assertThat(layer.source()).isEqualTo("minecraft:optifine/sky/world0/stars.png");
        assertThat(layer.blend()).isEqualTo(SkyLayer.Blend.ALPHA);
        assertThat(layer.angle(1)).isEqualTo(2);
        assertThat(layer.axisY()).isEqualTo(1);
        assertThat(SkyLayer.parse("rotate=false", FILE).angle(1)).isZero();
    }

    @Test
    void resolvesSourcesAsOptiFineDoes() {
        String folder = "optifine/sky/world0";
        assertThat(SkyLayer.resolve("sky2.png", folder)).isEqualTo("minecraft:optifine/sky/world0/sky2.png");
        assertThat(SkyLayer.resolve("~/sky/clouds.png", folder)).isEqualTo("minecraft:optifine/sky/clouds.png");
        assertThat(SkyLayer.resolve("/assets/minecraft/mcpatcher/sky/a.png", folder))
                .isEqualTo("minecraft:mcpatcher/sky/a.png");
        assertThat(SkyLayer.resolve("textures/sky/a.png", folder)).isEqualTo("minecraft:textures/sky/a.png");
        assertThat(SkyLayer.resolve("pack:sky/a.png", folder)).isEqualTo("pack:sky/a.png");
        assertThat(SkyLayer.resolve("Sky Night.png", folder)).isNull();
    }

    @Test
    void fadesInAndOutOverMidnight() {
        // Night stars: in from 18:00 to 19:00, out from 05:00 to 06:00.
        SkyLayer layer = SkyLayer.parse("startFadeIn=18:00\nendFadeIn=19:00\nstartFadeOut=5:00\nendFadeOut=6:00", FILE);
        assertThat(layer).isNotNull();
        assertThat(layer.brightness(SkyLayer.time("12:00"), 0, 0)).isZero();
        assertThat(layer.brightness(SkyLayer.time("18:30"), 0, 0)).isCloseTo(0.5F, within(0.01F));
        assertThat(layer.brightness(SkyLayer.time("0:00"), 0, 0)).isEqualTo(1);
        assertThat(layer.brightness(SkyLayer.time("5:30"), 0, 0)).isCloseTo(0.5F, within(0.01F));
    }

    @Test
    void anEndFadeOutLeftOutTakesAsLongAsTheFadeIn() {
        SkyLayer layer = SkyLayer.parse("startFadeIn=18:00\nendFadeIn=20:00\nstartFadeOut=4:00", FILE);
        assertThat(layer).isNotNull();
        assertThat(layer.brightness(SkyLayer.time("5:00"), 0, 0)).isCloseTo(0.5F, within(0.01F));
        assertThat(layer.brightness(SkyLayer.time("6:30"), 0, 0)).isZero();
    }

    @Test
    void halfTheFadeTimesIsNoLayer() {
        assertThat(SkyLayer.parse("startFadeIn=18:00", FILE)).isNull();
        assertThat(SkyLayer.parse("startFadeIn=dusk\nendFadeIn=19:00\nstartFadeOut=5:00", FILE))
                .isNull();
    }

    @Test
    void clearSkiesFadeWithTheRain() {
        SkyLayer clear = SkyLayer.parse("", FILE);
        assertThat(clear.brightness(0, 1, 0)).isZero();
        assertThat(clear.brightness(0, 0.25F, 0)).isEqualTo(0.75F);
        SkyLayer storm = SkyLayer.parse("weather=thunder", FILE);
        assertThat(storm.brightness(0, 0, 0)).isZero();
        assertThat(storm.brightness(0, 1, 1)).isEqualTo(1);
    }

    @Test
    void timesCountFromDawn() {
        assertThat(SkyLayer.time("6:00")).isZero();
        assertThat(SkyLayer.time("12:00")).isEqualTo(6000);
        assertThat(SkyLayer.time("0:00")).isEqualTo(18000);
        assertThat(SkyLayer.time("6:30")).isEqualTo(500);
        assertThat(SkyLayer.time("noon")).isEqualTo(-1);
    }

    @Test
    void ordersFilesByTheirNumber() {
        assertThat(SkyLayer.order("optifine/sky/world0/sky2.properties"))
                .isLessThan(SkyLayer.order("optifine/sky/world0/sky10.properties"));
        assertThat(SkyLayer.order("optifine/sky/world0/sun.properties")).isEqualTo(Integer.MAX_VALUE);
    }
}
