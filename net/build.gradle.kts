plugins {
    `java-library`
}

// The game's network transport under its Netty pipeline (ADR 0009): the Netty channel over the
// platform's connections, WebSocket addresses, and message framing. No game types.

dependencies {
    api(project(":platform-api"))

    // Compile against the Netty version the game ships (libs.versions.toml). At run time the game's
    // own copies are on the classpath, so these are compile-only.
    compileOnly(libs.netty.buffer)
    compileOnly(libs.netty.transport)
    compileOnly(libs.netty.codec.base)

    // TLS for Microsoft sign-in (incari.net.tls, ADR 0070): Bouncy Castle's lightweight TLS client,
    // which runs over any NetConnection, a Wisp stream included. Ships in both builds.
    implementation(libs.bctls)
    implementation(libs.bcutil)
    implementation(libs.bcprov)
    // Microsoft's, Xbox's and Mojang's JSON through Gson's tree API (no reflection). The game ships
    // Gson, so this is compile-only, as in :online.
    compileOnly(libs.gson)

    testImplementation(libs.gson)
    testImplementation(libs.bcpkix)
    testImplementation(libs.netty.buffer)
    testImplementation(libs.netty.transport)
    testImplementation(libs.netty.codec.base)
    testImplementation(platform(libs.junit.bom))
    testImplementation(libs.junit.jupiter)
    testImplementation(libs.assertj)
    testImplementation(libs.archunit)
    testRuntimeOnly(libs.junit.launcher)
}
