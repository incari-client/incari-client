plugins {
    `java-library`
}

// Schematics: a structure from a .litematic, .schem or .nbt file, placed in the world as ghost
// blocks to build by, with a material list and the wrong blocks marked (ADR 0066).
// A feature module: it sees the module framework and nothing else.
dependencies {
    api(project(":framework"))

    testImplementation(testFixtures(project(":framework")))
    testImplementation(platform(libs.junit.bom))
    testImplementation(libs.junit.jupiter)
    testImplementation(libs.assertj)
    testImplementation(libs.archunit)
    testRuntimeOnly(libs.junit.launcher)
}
