package incari.net.tls;

import java.util.Base64;
import java.util.List;
import org.bouncycastle.asn1.x509.Certificate;

/**
 * The root certificates Incari trusts for Microsoft sign-in: only the roots Microsoft's and Mojang's
 * sign-in hosts chain to, not a browser's whole store. Every host in {@code MicrosoftEndpoints}
 * chained to DigiCert Global Root G2 when this was written (November 2026); the other three are the
 * roots Microsoft names for its own services, so a move to them keeps sign-in working.
 *
 * <p>A host that moves to another root fails the handshake with "not issued by a trusted root" until
 * this list is updated. Embedded as DER rather than read as a resource: the browser build's class
 * path resources are the game's, mounted from its jar.
 */
final class TrustAnchors {

    /**
     * DigiCert Global Root G2 (RSA 2048, until 2038-01-15). SHA-256
     * CB:3C:CB:B7:60:31:E5:E0:13:8F:8D:D3:9A:23:F9:DE:47:FF:C3:5E:43:C1:14:4C:EA:27:D4:6A:5A:B1:CB:5F.
     */
    private static final String DIGICERT_GLOBAL_ROOT_G2 =
            "MIIDjjCCAnagAwIBAgIQAzrx5qcRqaC7KGSxHQn65TANBgkqhkiG9w0BAQsFADBhMQswCQYDVQQGEwJVUzEVMBMGA1UEChMM"
                    + "RGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3d3cuZGlnaWNlcnQuY29tMSAwHgYDVQQDExdEaWdpQ2VydCBHbG9iYWwgUm9vdCBH"
                    + "MjAeFw0xMzA4MDExMjAwMDBaFw0zODAxMTUxMjAwMDBaMGExCzAJBgNVBAYTAlVTMRUwEwYDVQQKEwxEaWdpQ2VydCBJbmMx"
                    + "GTAXBgNVBAsTEHd3dy5kaWdpY2VydC5jb20xIDAeBgNVBAMTF0RpZ2lDZXJ0IEdsb2JhbCBSb290IEcyMIIBIjANBgkqhkiG"
                    + "9w0BAQEFAAOCAQ8AMIIBCgKCAQEAuzfNNNx7a8myaJCtSnX/RrohCgiN9RlUyfuI2/Ou8jqJkTx65qsGGmvPrC3oXgkkRLpi"
                    + "mn7Wo6h+4FR1IAWsULecYxpsMNzaHxmx1x7e/dfgy5SDN67sH0NO3Xss0r0upS/kqbitOtSZpLYl6ZtrAGCSYP9PIUkY92eQ"
                    + "q2EGnI/yuum06ZIya7XzV+hdG82MHauVBJVJ8zUtluNJbd134/tJS7SsVQepj5WztCO7TG1F8PapspUwtP1MVYwnSlcUfIKd"
                    + "zXOS0xZKBgyMUNGPHgm+F6HmIcr9g+UQvIOlCsRnKPZzFBQ9RnbDhxSJITRNrw9FDKZJobq7nMWxM4MphQIDAQABo0IwQDAP"
                    + "BgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBhjAdBgNVHQ4EFgQUTiJUIBiV5uNu5g/6+rkS7QYXjzkwDQYJKoZIhvcN"
                    + "AQELBQADggEBAGBnKJRvDkhj6zHd6mcY1Yl9PMWLSn/pvtsrF9+wX3N3KjITOYFnQoQj8kVnNeyIv/iPsGEMNKSuIEyExtv4"
                    + "NeF22d+mQrvHRAiGfzZ0JFrabA0UWTW98kndth/Jsw1HKj2ZL7tcu7XUIOGZX1NGFdtom/DzMNU+MeKNhJ7jitralj41E6Vf"
                    + "8PlwUHBHQRFXGU7Aj64GxJUTFy8bJZ918rGOmaFvE7FBcf6IKshPECBV1/MUReXgRPTqh5Uykw7+U0b6LJ3/iyK5S9kJRaTe"
                    + "pLiaWN0bfVKfjllDiIGknibVb63dDcY3fe0Dkhvld1927jyNxF1WW6LZZm6zNTflMrY=";

    /**
     * DigiCert Global Root G3 (ECDSA P-384, until 2038-01-15). SHA-256
     * 31:AD:66:48:F8:10:41:38:C7:38:F3:9E:A4:32:01:33:39:3E:3A:18:CC:02:29:6E:F9:7C:2A:C9:EF:67:31:D0.
     */
    private static final String DIGICERT_GLOBAL_ROOT_G3 =
            "MIICPzCCAcWgAwIBAgIQBVVWvPJepDU1w6QP1atFcjAKBggqhkjOPQQDAzBhMQswCQYDVQQGEwJVUzEVMBMGA1UEChMMRGln"
                    + "aUNlcnQgSW5jMRkwFwYDVQQLExB3d3cuZGlnaWNlcnQuY29tMSAwHgYDVQQDExdEaWdpQ2VydCBHbG9iYWwgUm9vdCBHMzAe"
                    + "Fw0xMzA4MDExMjAwMDBaFw0zODAxMTUxMjAwMDBaMGExCzAJBgNVBAYTAlVTMRUwEwYDVQQKEwxEaWdpQ2VydCBJbmMxGTAX"
                    + "BgNVBAsTEHd3dy5kaWdpY2VydC5jb20xIDAeBgNVBAMTF0RpZ2lDZXJ0IEdsb2JhbCBSb290IEczMHYwEAYHKoZIzj0CAQYF"
                    + "K4EEACIDYgAE3afZu4q4C/sLfyHS8L6+c/MzXRq8NOrexpu80JX28MzQC7phW1FGfp4tn+6OYwwX7Adw9c+ELkCDnOg/QW07"
                    + "rdOkFFk2eJ0DQ+4QE2xy3q6Ip6FrtUPOZ9wj/wMco+I+o0IwQDAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBhjAd"
                    + "BgNVHQ4EFgQUs9tIpPmhxdiuNkHMEWNpYim8S8YwCgYIKoZIzj0EAwMDaAAwZQIxAK288mw/EkrRLTnDCgmXc/SINoyIJ7vm"
                    + "iI1Qhadj+Z4y3maTD/HMsQmP3Wyr+mt/oAIwOWZbwmSNuJ5Q3KjVSaLtx9zRSX8XAbjIho9OjIgrqJqpisXRAL34VOKa5Vt8"
                    + "sycX";

    /**
     * Microsoft RSA Root Certificate Authority 2017 (RSA 4096, until 2042-07-18). SHA-256
     * C7:41:F7:0F:4B:2A:8D:88:BF:2E:71:C1:41:22:EF:53:EF:10:EB:A0:CF:A5:E6:4C:FA:20:F4:18:85:30:73:E0.
     */
    private static final String MICROSOFT_RSA_ROOT_2017 =
            "MIIFqDCCA5CgAwIBAgIQHtOXCV/YtLNHcB6qvn9FszANBgkqhkiG9w0BAQwFADBlMQswCQYDVQQGEwJVUzEeMBwGA1UEChMV"
                    + "TWljcm9zb2Z0IENvcnBvcmF0aW9uMTYwNAYDVQQDEy1NaWNyb3NvZnQgUlNBIFJvb3QgQ2VydGlmaWNhdGUgQXV0aG9yaXR5"
                    + "IDIwMTcwHhcNMTkxMjE4MjI1MTIyWhcNNDIwNzE4MjMwMDIzWjBlMQswCQYDVQQGEwJVUzEeMBwGA1UEChMVTWljcm9zb2Z0"
                    + "IENvcnBvcmF0aW9uMTYwNAYDVQQDEy1NaWNyb3NvZnQgUlNBIFJvb3QgQ2VydGlmaWNhdGUgQXV0aG9yaXR5IDIwMTcwggIi"
                    + "MA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQDKW76UM4wplZEWCpW9R2LBifOZNt9GkMml7Xhqb0eRaPgnZ1AzHaGm++Dl"
                    + "Q6OEAlcBXZxIQIJTELy/xztokLaCLeX0ZdDMbRnMlfl7rEqUrQ7eS0MdhweSE5CAg2Q1OQT85elss7YfUJQ4ZVBcF0a5toW1"
                    + "HLUX6NZFndiyJrDKxHBKrmCk3bPZ7Pw71VdyvD/IybLeS2v4I2wDwAW9lcfNcztmgGTjGqwu+UcF8ga2m3P1eDNbx6H7Jyqh"
                    + "tJqRjJHTOoI+dkC0zVJhUXAoP8XFWvLJjEm7FFtNyP9nTUwSlq31/niol4fX/V4ggNyhSyL71Imtus5Hl0dVe49FyGcohJUc"
                    + "aDDv70ngNXtk55iwlNpNhTs+VcQor1fznhPbRiefHqJeRIOkpcrVE7NLP8TjwuaGYaRSMLl6IE9vDzhTyzMMEyuP1pq9Ksgt"
                    + "sRx9S1HKR9FIJ3Jdh+vVReZIZZ2vUpC6W6IYZVcSn2i51BVrlMRpIpj0M+Dt+VGOQVDJNE92kKz8OMHY4Xu54+OU4UZpyw4K"
                    + "UGsTuqwPN1q3ErWQgR5WrlcihtnJ0tHXUeOrO8ZV/R4O03QK0dqq6mm4lyiPSMQH+FJDOvTKVTUssKZqwJz58oHhEmrARdln"
                    + "s87/I6KJClTUFLkqqNfs+avNJVgyeY+QW5g5xAgGwax/Dj0ApQIDAQABo1QwUjAOBgNVHQ8BAf8EBAMCAYYwDwYDVR0TAQH/"
                    + "BAUwAwEB/zAdBgNVHQ4EFgQUCctZf4aycI8awznjwNnpv7tNsiMwEAYJKwYBBAGCNxUBBAMCAQAwDQYJKoZIhvcNAQEMBQAD"
                    + "ggIBAKyvPl3CEZaJjqPnktaXFbgToqZCLgLNFgVZJ8og6Lq46BrsTaiXVq5lQ7GPAJtSzVXNUzltYkyLDVt8LkS/gxCP81OC"
                    + "gMNPOsduET/m4xaRhPtthH80dK2Jp86519efhGSSvpWhrQlTM93uCupKUY5vVau6tZRGrox/2KJQJWVggEbbMwSubLWYdFQl"
                    + "3JPk+ONVFT24bcMKpBLBaYVu32TxU5nhSnUgnZUP5NbcA/FZGOhHibJXWpS2qdgXKxdJ5XbLwVaZOjex/2kskZGT4d9Mozd2"
                    + "TaGf+G0eHdP67Pv0RR0Tbc/3WeUiJ3IrhvNXuzDtJE3cfVa7o7P4NHmJweDyAmH3pvwPuxwXC65B2Xy9J6P9LjrRk5Sxcx0k"
                    + "i69bIImtt2dmefU6xqaWM/5TkshGsRGRxpl/j8nWZjEgQRCHLQzWwa80mMpkg/sTV9HB8Dx6jKXB/ZUhoHHBk2dxEuqPiApp"
                    + "GWSZI1b7rCoucL5mxAyE7+WL85MB+GqQk2dLsmijtWKP6T+MejteD+eMuMZ87zf9dOLITzNy4ZQ5bb0Sr74MTnB8G2+NszKT"
                    + "c0QWbej09+CVgI+WXTik9KveCjCHk9hNAHFiRSdLOkKEW39lt2c0Ui2cFmuqqNh7o0JMcccMyj6D5KbvtwEwXlGjefVwaaZB"
                    + "RA+GsCyRxj3qrg+E";

    /**
     * Microsoft ECC Root Certificate Authority 2017 (ECDSA P-384, until 2042-07-18). SHA-256
     * 35:8D:F3:9D:76:4A:F9:E1:B7:66:E9:C9:72:DF:35:2E:E1:5C:FA:C2:27:AF:6A:D1:D7:0E:8E:4A:6E:DC:BA:02.
     */
    private static final String MICROSOFT_ECC_ROOT_2017 =
            "MIICWTCCAd+gAwIBAgIQZvI9r4fei7FK6gxXMQHC7DAKBggqhkjOPQQDAzBlMQswCQYDVQQGEwJVUzEeMBwGA1UEChMVTWlj"
                    + "cm9zb2Z0IENvcnBvcmF0aW9uMTYwNAYDVQQDEy1NaWNyb3NvZnQgRUNDIFJvb3QgQ2VydGlmaWNhdGUgQXV0aG9yaXR5IDIw"
                    + "MTcwHhcNMTkxMjE4MjMwNjQ1WhcNNDIwNzE4MjMxNjA0WjBlMQswCQYDVQQGEwJVUzEeMBwGA1UEChMVTWljcm9zb2Z0IENv"
                    + "cnBvcmF0aW9uMTYwNAYDVQQDEy1NaWNyb3NvZnQgRUNDIFJvb3QgQ2VydGlmaWNhdGUgQXV0aG9yaXR5IDIwMTcwdjAQBgcq"
                    + "hkjOPQIBBgUrgQQAIgNiAATUvD0CQnVBEyPNgASGAlEvaqiBYgtlzPbKnR5vSmZRogPZnZH6thaxjG7efM3beaYvzrvOcS/l"
                    + "paso7GMEZpn4+vKTEAXhgShC48Zo9OYbhGBKia/teQ87zvH2RPUBeMCjVDBSMA4GA1UdDwEB/wQEAwIBhjAPBgNVHRMBAf8E"
                    + "BTADAQH/MB0GA1UdDgQWBBTIy5lycFIM+Oa+sgRXKSrPQhDtNTAQBgkrBgEEAYI3FQEEAwIBADAKBggqhkjOPQQDAwNoADBl"
                    + "AjBY8k3qDPlfXu5gKcs68tvWMoQZP3zVL8KxzJOuULsJMsbG7X7JNpQS5GiFBqIb0C8CMQCZ6Ra0DvpWSNSkMBaReNtUjGUB"
                    + "iudQZsIxtzm6uBoiB078a1QWIP8rtedMDE2mT3M=";

    private static volatile List<Certificate> roots = List.of();

    private TrustAnchors() {}

    /** The pinned roots, parsed once. */
    static List<Certificate> roots() {
        List<Certificate> parsed = roots;
        if (parsed.isEmpty()) {
            parsed = List.of(
                    parse(DIGICERT_GLOBAL_ROOT_G2),
                    parse(DIGICERT_GLOBAL_ROOT_G3),
                    parse(MICROSOFT_RSA_ROOT_2017),
                    parse(MICROSOFT_ECC_ROOT_2017));
            roots = parsed;
        }
        return parsed;
    }

    private static Certificate parse(String base64) {
        return Certificate.getInstance(Base64.getDecoder().decode(base64));
    }
}
