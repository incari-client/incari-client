import net.ltgt.gradle.errorprone.errorprone

plugins {
    `java-library`
}

// The JDK classes and methods TeaVM's class library lacks, for the browser build only.
// tools/webbuild wires them in by name (ClasslibPolicy, AdditionsTransformer, RemovalTransformer).

tasks.withType<JavaCompile>().configureEach {
    // This module implements JDK APIs as they are, so two checks don't apply: throwing is the
    // documented behaviour of many stand-ins (a page can't do what they ask), and "obsolete" JDK
    // types (SortedMap, Date...) are what the game and its libraries call.
    options.errorprone.disable("DoNotCallSuggester", "JdkObsolete")
}

dependencies {
    // The logging API the game uses; the browser build binds it to the console (logging/).
    compileOnly(libs.slf4j.api)
    compileOnly(libs.jopt.simple)
    // Linear memory (Address) and the platform marker that picks it, in memory/Heap.
    compileOnly(libs.teavm.interop)
    // Netty's logger lookup, which additions/ binds to SLF4J like the game's own logging.
    compileOnly(libs.netty.common)
    // Via's pipeline helpers, substituted (substitutes/com/viaversion).
    compileOnly(libs.netty.buffer)
    compileOnly(libs.netty.transport)
    compileOnly(libs.netty.codec.base)
    // @JSBody, for the few figures only the page knows (memory/PageHeap).
    compileOnly(libs.teavm.jso)
    // The deflate engine under the Deflater substitute; TeaVM's own zip classes use it too.
    compileOnly(libs.jzlib)
    // AES, RSA and SHA256withRSA behind Cipher, KeyFactory and Signature (security/, ADR 0070):
    // Bouncy Castle's lightweight engines, which :net already ships in the build for sign-in's TLS.
    implementation(libs.bcprov)
    testImplementation(libs.jzlib)
    testImplementation(libs.bcprov)
    testImplementation(libs.slf4j.api)

    testImplementation(platform(libs.junit.bom))
    testImplementation(libs.junit.jupiter)
    testImplementation(libs.assertj)
    testRuntimeOnly(libs.junit.launcher)
}
