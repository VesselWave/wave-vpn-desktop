// SPDX-License-Identifier: GPL-3.0-or-later
package me.vwave.vpn.core
import org.junit.Assert.*
import org.junit.Test
// Same invented network datasets as best-networks.json. No live network access.
class BestQualityTest {
 data class Case(val name:String,val pings:Map<String,Int>,val loads:Map<String,Int>,val users:Map<String,Int>,val expected:String)
 @Test fun representativeChoices() {
 val cases=listOf(Case("Chicago",mapOf("US" to 24,"NL" to 110,"SG" to 230,"CA" to 38,"JP" to 165),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),"US"),
Case("Amsterdam",mapOf("US" to 105,"NL" to 12,"SG" to 165,"CA" to 110,"JP" to 225),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),"NL"),
Case("Singapore",mapOf("US" to 225,"NL" to 165,"SG" to 12,"CA" to 235,"JP" to 75),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),"SG"),
Case("Tokyo",mapOf("US" to 150,"NL" to 225,"SG" to 75,"CA" to 155,"JP" to 14),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),"JP"),
Case("Singapore-congested",mapOf("US" to 225,"NL" to 165,"SG" to 12,"CA" to 235,"JP" to 75),mapOf("US" to 20,"NL" to 10,"SG" to 80,"CA" to 20,"JP" to 20),mapOf("US" to 20,"NL" to 10,"SG" to 80,"CA" to 20,"JP" to 20),"JP"),
Case("US-unreachable",mapOf("US" to -1,"NL" to 110,"SG" to 230,"CA" to 38,"JP" to 165),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),"CA"),
Case("US-partial",mapOf("US" to 24,"CA" to 38),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),"US"),
Case("SG-missing",mapOf("US" to 225,"NL" to 165,"CA" to 235,"JP" to 75),mapOf("US" to 20,"NL" to 10,"SG" to 80,"CA" to 20,"JP" to 20),mapOf("US" to 20,"NL" to 10,"SG" to 80,"CA" to 20,"JP" to 20),"JP"),
Case("no-measurements",mapOf(),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),mapOf("US" to 30,"NL" to 30,"SG" to 30,"CA" to 30,"JP" to 30),"CA"),
Case("Singapore-load-policy-divergence",mapOf("US" to 225,"NL" to 165,"SG" to 12,"CA" to 235,"JP" to 75),mapOf("US" to 90,"NL" to 10,"SG" to 80,"CA" to 90,"JP" to 90),mapOf("US" to 90,"NL" to 10,"SG" to 80,"CA" to 90,"JP" to 90),"SG"))
 val base=FakeAirNetwork.fixtureServers().first()
 for(c in cases) {
 val fleet=c.loads.map { (n,l) -> base.copy(name=n,countryCode=n,location=n,bandwidthMax=1000,bandwidth=l*625000L,users=c.users.getValue(n),usersMax=100,scoreBase=0,warningOpen=null,warningClosed=null) }
 val selected=Scoring.rank(fleet,c.pings,Factors()).first()
 val fastest=c.pings.values.filter{it>0}.minOrNull()
 val regret=c.pings[selected.name]?.takeIf{it>0}?.let{it-(fastest?:it)}
 println("${c.name} Android=${selected.name} RTT-regret=$regret")
 assertEquals(c.name,c.expected,selected.name)
 // Country/server preferences constrain the candidate list even against global Best.
 assertEquals("NL",Scoring.rank(fleet,c.pings,Factors(),include={it.countryCode=="NL"}).first().name)
 }
 }
 @Test fun recordedFiveLocationSample() {
 val base=FakeAirNetwork.fixtureServers().first()
 val fleet=listOf(base.copy(name="Guniibuu",bandwidthMax=20000,bandwidth=225000000L,users=392,usersMax=150,scoreBase=0,warningOpen=null,warningClosed=null),
base.copy(name="Kornephoros",bandwidthMax=20000,bandwidth=362500000L,users=376,usersMax=150,scoreBase=0,warningOpen=null,warningClosed=null),
base.copy(name="Piautos",bandwidthMax=20000,bandwidth=437500000L,users=412,usersMax=150,scoreBase=0,warningOpen=null,warningClosed=null),
base.copy(name="Luyten",bandwidthMax=2000,bandwidth=47500000L,users=143,usersMax=100,scoreBase=1000,warningOpen=null,warningClosed=null),
base.copy(name="Taphao",bandwidthMax=2000,bandwidth=47500000L,users=130,usersMax=100,scoreBase=1000,warningOpen=null,warningClosed=null))
 assertEquals("Guniibuu",Scoring.rank(fleet,mapOf("Guniibuu" to 26,"Kornephoros" to 35,"Piautos" to 113,"Luyten" to 194,"Taphao" to 129),Factors()).first().name)
 }
}
