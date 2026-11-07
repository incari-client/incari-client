package incari.client.skins;

import static org.assertj.core.api.Assertions.assertThat;

import org.junit.jupiter.api.Test;

class SkinDownloadsTest {

    @Test
    void fetchesMojangsHttpTextureUrlsOverHttps() {
        assertThat(SkinDownloads.secure("http://textures.minecraft.net/texture/abc"))
                .isEqualTo("https://textures.minecraft.net/texture/abc");
        assertThat(SkinDownloads.secure("HTTP://textures.minecraft.net/texture/abc"))
                .isEqualTo("https://textures.minecraft.net/texture/abc");
    }

    @Test
    void leavesHttpsUrlsAlone() {
        assertThat(SkinDownloads.secure("https://textures.minecraft.net/texture/abc"))
                .isEqualTo("https://textures.minecraft.net/texture/abc");
    }
}
