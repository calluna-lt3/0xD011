file hosting service because every one i ever use dies

for POST reqs:
* used by friends only
    * username/password as fields in POST reqs for establishing new
      connections? then can whitelist the IP afterwards
* need to block spam
    * set # of queries before blocked via firewall, idk how to do this

how do we handle page routing?
* hardcode all knowable paths, e.g. "/", "/styles.css"
* route ALL NON-DEFINED TRAFFIC to some "/arbitrary/" path so that we can only
  serve files that we want
* for now just serve files on the paths that match their names, however in the
  future we can do a lookup from path -> hashed filename if we want more
  memorable names
