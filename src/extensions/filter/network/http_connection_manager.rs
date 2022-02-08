// Copyright 2021 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::sync::Arc;

use envoy_control_plane::envoy::extensions::filters::network::http_connection_manager::v3::{
    http_connection_manager::RouteSpecifier, HttpConnectionManager as V3HttpConnectionManager,
};

use crate::cluster::{Cluster, Clusters};
use crate::route::{Action, ClusterSpecifier, RouteAction};
use crate::Request;

#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum Error {
    #[error("virtual host's domain is invalid: {0}")]
    BadDomainGlob(String),
    #[error("TODO: only static route_config is supported for now")]
    UnsupportedRouteConfig,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct VirtualHost {
    name: String,
    domains: Vec<glob::Pattern>,
    routes: Vec<crate::route::Route>,
}

#[derive(Debug, Default, Clone)]
pub struct HttpConnectionManager {
    virtual_hosts: Vec<VirtualHost>,
    clusters: Arc<Clusters>,
}

impl HttpConnectionManager {
    pub fn get_cluster(&self, req: &Request) -> Option<Arc<Cluster>> {
        // TODO: does Host header even work for H2?
        if let Some(authority) = req.headers().get("Host") {
            if let Ok(authority) = authority.to_str() {
                for vh in self.virtual_hosts.iter() {
                    if !vh.domains.iter().any(|domain| domain.matches(authority)) {
                        // authority didn't match any of our virtual host domains; bail
                        continue;
                    }
                    for route in vh.routes.iter() {
                        if let Some(Action::Route(RouteAction {
                            cluster: ClusterSpecifier::Name(cluster_name),
                        })) = route.matches(req.uri())
                        {
                            let clusters = self.clusters.load();
                            return clusters.get(cluster_name).cloned();
                        }
                    }
                }
            }
        }
        None
    }
}

impl TryFrom<(V3HttpConnectionManager, Arc<Clusters>)> for HttpConnectionManager {
    type Error = Error;

    fn try_from(
        (v3_conn_mgr, clusters): (V3HttpConnectionManager, Arc<Clusters>),
    ) -> Result<Self, Self::Error> {
        if let Some(RouteSpecifier::RouteConfig(route_cfg)) = v3_conn_mgr.route_specifier {
            let mut domain_err: Option<Error> = None;
            let virtual_hosts = route_cfg
                .virtual_hosts
                .into_iter()
                .map(|v_host| VirtualHost {
                    name: v_host.name,
                    domains: v_host
                        .domains
                        .into_iter()
                        .filter_map(|domain| match glob::Pattern::new(&domain) {
                            Ok(pattern) => Some(pattern),
                            Err(_) => {
                                domain_err = Some(Error::BadDomainGlob(domain));
                                None
                            }
                        })
                        .collect(),
                    routes: v_host
                        .routes
                        .into_iter()
                        .filter_map(|route| crate::route::Route::try_from(route).ok())
                        .collect(),
                })
                .collect::<Vec<_>>();
            // if we had a problem with the domain above, fail.
            if let Some(err) = domain_err {
                return Err(err);
            }
            Ok(HttpConnectionManager {
                virtual_hosts,
                clusters,
            })
        } else {
            Err(Error::UnsupportedRouteConfig)
        }
    }
}
