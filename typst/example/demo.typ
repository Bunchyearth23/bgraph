#import "../bgraph.typ": diagram

= A diagram from source in the document

#diagram("
  graph { title: \"From Typst\" }
  node ui \"Browser\"
  node api \"API\"
  layout {
    place ui at (0, 0)
    place api right-of ui gap 50
  }
  ui -> api \"HTTPS\"
", width: 70%)

A diagram that does not parse fails the build rather than leaving a hole —
uncomment the line below to see the compiler's own message in place.

// #diagram("node ??? broken")
