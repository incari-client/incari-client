package incari.net.msa;

import java.util.Set;

/**
 * Where Microsoft sign-in goes, and as whom (ADR 0070). Every URL here is HTTPS on a host in
 * {@link #HOSTS}, the only hosts the sign-in client will connect to.
 */
public final class MicrosoftEndpoints {

    /**
     * The OAuth client sign-in asks Microsoft for tokens as: the official Minecraft launcher's. An
     * app of Incari's own would need Mojang's approval to reach {@code login_with_xbox}. Swapping it
     * is this one line plus {@link #SCOPE} (an Azure app uses {@code login.microsoftonline.com}).
     */
    public static final String CLIENT_ID = "00000000402b5328";

    /** The scope the launcher's client asks for: an Xbox Live ticket. */
    public static final String SCOPE = "service::user.auth.xboxlive.com::MBI_SSL";

    static final String DEVICE_CODE = "https://login.live.com/oauth20_connect.srf";
    static final String TOKEN = "https://login.live.com/oauth20_token.srf";
    static final String XBOX_USER = "https://user.auth.xboxlive.com/user/authenticate";
    static final String XSTS = "https://xsts.auth.xboxlive.com/xsts/authorize";
    static final String LOGIN_WITH_XBOX = "https://api.minecraftservices.com/authentication/login_with_xbox";
    static final String PROFILE = "https://api.minecraftservices.com/minecraft/profile";
    static final String CERTIFICATES = "https://api.minecraftservices.com/player/certificates";
    static final String JOIN = "https://sessionserver.mojang.com/session/minecraft/join";

    /** The only hosts sign-in may reach. */
    public static final Set<String> HOSTS = Set.of(
            "login.live.com",
            "user.auth.xboxlive.com",
            "xsts.auth.xboxlive.com",
            "api.minecraftservices.com",
            "sessionserver.mojang.com");

    /** Where the player types the code; the device-code answer must name a page on this host. */
    static final Set<String> CODE_PAGE_HOSTS = Set.of("www.microsoft.com", "microsoft.com");

    private MicrosoftEndpoints() {}
}
