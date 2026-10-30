// post.nc — one burial, and as many worlds as you care to try.
//
// `env` is stratum 2 and `--declare` is how a rite answers it, so the world
// this program runs against fits on the command line. Two exhumings of one
// trace reach two sealed traces, and both keep their names.

Str@2 who = must(descend env { env("WHO") });

U0 greet()
{
  concat("Hello, ", who);
}

demand greet();
