package incari.net.msa;

/**
 * A device-code sign-in under way: the player opens {@link #pageUrl} on any device and types
 * {@link #userCode}, while the client asks every {@link #intervalSeconds} whether they have.
 *
 * @param userCode the short code the player types
 * @param deviceCode the code the client polls with; secret, never shown or logged
 * @param pageUrl Microsoft's page for the code, checked to be on {@code microsoft.com}
 * @param expiresAtMillis when the code stops working, in epoch milliseconds
 * @param intervalSeconds how often to poll
 */
public record DeviceCode(
        String userCode, String deviceCode, String pageUrl, long expiresAtMillis, int intervalSeconds) {

    /** The page with the code filled in, for a link or a QR code. */
    public String pageWithCode() {
        return pageUrl + (pageUrl.indexOf('?') < 0 ? "?" : "&") + "otc=" + userCode;
    }

    @Override
    public String toString() {
        return "DeviceCode[" + userCode + ", " + pageUrl + "]";
    }
}
